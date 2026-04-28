import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "@/stores/appStore";
import type { VaultInfo } from "@/types";

interface CredentialSummary {
  id: string;
  name: string;
  credential_type: string;
  username: string | null;
  tags: string[];
  created_at: string;
  updated_at: string;
}

interface VaultState {
  vaults: VaultInfo[];
  activeVaultId: string | null;
  vaultLockStates: Record<string, boolean>;
  credentials: CredentialSummary[];
  loading: boolean;
  error: string | null;

  /** True when no vaults are unlocked for the active profile */
  vaultLocked: boolean;

  listVaults: () => Promise<void>;
  createVault: (password: string, name?: string, isDefault?: boolean) => Promise<VaultInfo>;
  unlockVault: (vaultId: string, password: string) => Promise<void>;
  lockVault: (vaultId: string) => Promise<void>;
  lockAllVaults: () => Promise<void>;
  deleteVault: (vaultId: string, password: string) => Promise<void>;
  shareVault: (vaultId: string, password: string, targetProfileId: string) => Promise<void>;
  unshareVault: (vaultId: string, targetProfileId: string) => Promise<void>;
  setActiveVaultId: (vaultId: string) => void;

  fetchCredentials: () => Promise<void>;
  addCredential: (request: { name: string; credential_type: string; username?: string; data: unknown; tags?: string[]; notes?: string }) => Promise<string>;
  getCredential: (id: string) => Promise<{ credential_type: string; username: string | null; data: Record<string, unknown> } | null>;
  updateCredential: (id: string, request: { name?: string; username?: string; data?: unknown; tags?: string[]; notes?: string }) => Promise<void>;
  deleteCredential: (id: string) => Promise<void>;

  /** Mark a specific vault as locked (called from auto-lock events) */
  markVaultLocked: (vaultId: string) => void;

  clearError: () => void;
}

function getProfileId(): string {
  return useAppStore.getState().activeProfileId ?? "default";
}

export const useVaultStore = create<VaultState>((set, get) => ({
  vaults: [],
  activeVaultId: null,
  vaultLockStates: {},
  credentials: [],
  loading: false,
  error: null,
  vaultLocked: true,

  listVaults: async () => {
    try {
      const rawVaults = await invoke<VaultInfo[]>("vault_list", { profileId: getProfileId() });
      const vaults: VaultInfo[] = Array.isArray(rawVaults) ? rawVaults : [];
      const lockStates: Record<string, boolean> = {};
      for (const v of vaults) {
        try {
          const locked = await invoke<boolean>("vault_is_locked", { vaultId: v.id });
          lockStates[v.id] = locked;
        } catch {
          lockStates[v.id] = true;
        }
      }
      const anyUnlocked = Object.values(lockStates).some((l) => !l);
      const activeId = get().activeVaultId;
      const resolvedActive = activeId && !lockStates[activeId]
        ? activeId
        : vaults.find((v) => !lockStates[v.id])?.id ?? vaults[0]?.id ?? null;
      set({ vaults, vaultLockStates: lockStates, vaultLocked: !anyUnlocked, activeVaultId: resolvedActive });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  createVault: async (password, name, isDefault) => {
    set({ loading: true, error: null });
    try {
      const info = await invoke<VaultInfo>("vault_create", {
        profileId: getProfileId(),
        masterPassword: password,
        name: name ?? undefined,
        isDefault: isDefault ?? undefined,
      });
      const lockStates = { ...get().vaultLockStates, [info.id]: false };
      set((s) => ({
        vaults: [...s.vaults, info],
        vaultLockStates: lockStates,
        activeVaultId: s.activeVaultId ?? info.id,
        vaultLocked: false,
        loading: false,
      }));
      return info;
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  unlockVault: async (vaultId, password) => {
    set({ loading: true, error: null });
    try {
      await invoke("vault_unlock", { vaultId, masterPassword: password });
      const lockStates = { ...get().vaultLockStates, [vaultId]: false };
      set({ vaultLockStates: lockStates, vaultLocked: false, activeVaultId: vaultId, loading: false });
      await get().fetchCredentials();
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  lockVault: async (vaultId) => {
    try {
      await invoke("vault_lock", { vaultId });
      get().markVaultLocked(vaultId);
    } catch (e) {
      set({ error: String(e) });
    }
  },

  lockAllVaults: async () => {
    try {
      await invoke("vault_lock_all");
      const lockStates: Record<string, boolean> = {};
      for (const v of get().vaults) {
        lockStates[v.id] = true;
      }
      set({ vaultLockStates: lockStates, vaultLocked: true, credentials: [] });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  deleteVault: async (vaultId, password) => {
    set({ loading: true, error: null });
    try {
      await invoke("vault_delete", { vaultId, password });
      const newLockStates = { ...get().vaultLockStates };
      delete newLockStates[vaultId];
      const newVaults = get().vaults.filter((v) => v.id !== vaultId);
      const newActive = get().activeVaultId === vaultId
        ? newVaults.find((v) => !newLockStates[v.id])?.id ?? newVaults[0]?.id ?? null
        : get().activeVaultId;
      set({ vaults: newVaults, vaultLockStates: newLockStates, activeVaultId: newActive, loading: false });
      await get().fetchCredentials();
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  shareVault: async (vaultId, password, targetProfileId) => {
    set({ loading: true, error: null });
    try {
      await invoke("vault_share", { vaultId, password, targetProfileId });
      set({ loading: false });
      await get().listVaults();
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  unshareVault: async (vaultId, targetProfileId) => {
    set({ loading: true, error: null });
    try {
      await invoke("vault_unshare", { vaultId, targetProfileId });
      set({ loading: false });
      await get().listVaults();
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  setActiveVaultId: (vaultId) => {
    set({ activeVaultId: vaultId });
    get().fetchCredentials();
  },

  fetchCredentials: async () => {
    const { activeVaultId, vaultLockStates } = get();
    if (!activeVaultId || vaultLockStates[activeVaultId]) return;
    set({ loading: true, error: null });
    try {
      const credentials = await invoke<CredentialSummary[]>("credential_list", { vaultId: activeVaultId });
      set({ credentials, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  addCredential: async (request) => {
    const { activeVaultId } = get();
    if (!activeVaultId) throw new Error("No active vault");
    set({ loading: true, error: null });
    try {
      const id = await invoke<string>("credential_create", { vaultId: activeVaultId, request });
      await get().fetchCredentials();
      set({ loading: false });
      return id;
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  getCredential: async (id) => {
    const { activeVaultId, vaultLockStates } = get();
    if (!activeVaultId || vaultLockStates[activeVaultId]) return null;
    try {
      const detail = await invoke<{
        credential_type: string;
        username: string | null;
        data: Record<string, unknown>;
      }>("credential_get", { vaultId: activeVaultId, id });
      return detail;
    } catch {
      return null;
    }
  },

  updateCredential: async (id, request) => {
    const { activeVaultId } = get();
    if (!activeVaultId) throw new Error("No active vault");
    set({ loading: true, error: null });
    try {
      await invoke("credential_update", { vaultId: activeVaultId, id, request });
      await get().fetchCredentials();
      set({ loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  deleteCredential: async (id) => {
    const { activeVaultId } = get();
    if (!activeVaultId) throw new Error("No active vault");
    set({ loading: true, error: null });
    try {
      await invoke("credential_delete", { vaultId: activeVaultId, id });
      set((state) => ({
        credentials: state.credentials.filter((c) => c.id !== id),
        loading: false,
      }));
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  markVaultLocked: (vaultId) => {
    const lockStates = { ...get().vaultLockStates, [vaultId]: true };
    const anyUnlocked = Object.values(lockStates).some((l) => !l);
    const newActive = get().activeVaultId === vaultId
      ? get().vaults.find((v) => !lockStates[v.id])?.id ?? null
      : get().activeVaultId;
    set({
      vaultLockStates: lockStates,
      vaultLocked: !anyUnlocked,
      activeVaultId: newActive,
      credentials: get().activeVaultId === vaultId ? [] : get().credentials,
    });
  },

  clearError: () => set({ error: null }),
}));
