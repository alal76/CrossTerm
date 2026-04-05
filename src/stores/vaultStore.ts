import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "@/stores/appStore";

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
  vaultLocked: boolean;
  credentials: CredentialSummary[];
  loading: boolean;
  error: string | null;

  checkVaultExists: () => Promise<boolean>;
  createVault: (password: string) => Promise<void>;
  unlockVault: (password: string) => Promise<void>;
  lockVault: () => Promise<void>;

  fetchCredentials: () => Promise<void>;
  addCredential: (request: { name: string; credential_type: string; username?: string; data: unknown; tags?: string[]; notes?: string }) => Promise<string>;
  updateCredential: (id: string, request: { name?: string; username?: string; data?: unknown; tags?: string[]; notes?: string }) => Promise<void>;
  deleteCredential: (id: string) => Promise<void>;

  clearError: () => void;
}

function getProfileId(): string {
  return useAppStore.getState().activeProfileId ?? "default";
}

export const useVaultStore = create<VaultState>((set, get) => ({
  vaultLocked: true,
  credentials: [],
  loading: false,
  error: null,

  checkVaultExists: async () => {
    try {
      await invoke<boolean>("vault_is_locked");
      return true;
    } catch {
      return false;
    }
  },

  createVault: async (password) => {
    set({ loading: true, error: null });
    try {
      await invoke("vault_create", { profileId: getProfileId(), masterPassword: password });
      set({ vaultLocked: false, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  unlockVault: async (password) => {
    set({ loading: true, error: null });
    try {
      await invoke("vault_unlock", { profileId: getProfileId(), masterPassword: password });
      set({ vaultLocked: false, loading: false });
      await get().fetchCredentials();
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  lockVault: async () => {
    try {
      await invoke("vault_lock");
      set({ vaultLocked: true, credentials: [] });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  fetchCredentials: async () => {
    if (get().vaultLocked) return;
    set({ loading: true, error: null });
    try {
      const credentials = await invoke<CredentialSummary[]>("credential_list");
      set({ credentials, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  addCredential: async (request) => {
    set({ loading: true, error: null });
    try {
      const id = await invoke<string>("credential_create", { request });
      await get().fetchCredentials();
      set({ loading: false });
      return id;
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  updateCredential: async (id, request) => {
    set({ loading: true, error: null });
    try {
      await invoke("credential_update", { id, request });
      await get().fetchCredentials();
      set({ loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  deleteCredential: async (id) => {
    set({ loading: true, error: null });
    try {
      await invoke("credential_delete", { id });
      set((state) => ({
        credentials: state.credentials.filter((c) => c.id !== id),
        loading: false,
      }));
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  clearError: () => set({ error: null }),
}));
