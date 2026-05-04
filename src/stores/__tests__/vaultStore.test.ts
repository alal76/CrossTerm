import { describe, it, expect, beforeEach, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useVaultStore } from "@/stores/vaultStore";

const mockInvoke = vi.mocked(invoke);

const MOCK_VAULT = {
  id: "vault-1",
  name: "Default",
  is_default: true,
  owner_profile_id: "default",
  shared_with: [] as string[],
  created_at: new Date().toISOString(),
};

const MOCK_VAULT_2 = {
  id: "vault-2",
  name: "Work",
  is_default: false,
  owner_profile_id: "default",
  shared_with: [] as string[],
  created_at: new Date().toISOString(),
};

function resetStore() {
  useVaultStore.setState({
    vaults: [],
    activeVaultId: null,
    vaultLockStates: {},
    vaultLocked: true,
    credentials: [],
    loading: false,
    error: null,
  });
}

describe("vaultStore", () => {
  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  // ── Initial state ──

  describe("initial state", () => {
    it("has correct initial values", () => {
      const state = useVaultStore.getState();
      expect(state.vaultLocked).toBe(true);
      expect(state.vaults).toEqual([]);
      expect(state.credentials).toEqual([]);
      expect(state.loading).toBe(false);
      expect(state.error).toBeNull();
      expect(state.activeVaultId).toBeNull();
    });
  });

  // ── listVaults ──

  describe("listVaults", () => {
    it("calls invoke('vault_list') and populates vaults", async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "vault_list") return [MOCK_VAULT];
        if (cmd === "vault_is_locked") return true;
        return undefined;
      });

      await useVaultStore.getState().listVaults();

      expect(mockInvoke).toHaveBeenCalledWith("vault_list", { profileId: "default" });
      expect(useVaultStore.getState().vaults).toHaveLength(1);
      expect(useVaultStore.getState().vaults[0].id).toBe("vault-1");
    });

    it("leaves vaults as [] when vault_list returns empty array", async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "vault_list") return [];
        return undefined;
      });

      await useVaultStore.getState().listVaults();

      expect(useVaultStore.getState().vaults).toEqual([]);
    });

    it("sets vaultLocked to false when at least one vault is unlocked", async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "vault_list") return [MOCK_VAULT];
        if (cmd === "vault_is_locked") return false; // unlocked
        return undefined;
      });

      await useVaultStore.getState().listVaults();

      expect(useVaultStore.getState().vaultLocked).toBe(false);
    });

    it("sets vaultLocked to true when all vaults are locked", async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "vault_list") return [MOCK_VAULT];
        if (cmd === "vault_is_locked") return true;
        return undefined;
      });

      await useVaultStore.getState().listVaults();

      expect(useVaultStore.getState().vaultLocked).toBe(true);
    });

    it("sets error when vault_list throws", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("network error"));

      await useVaultStore.getState().listVaults();

      expect(useVaultStore.getState().error).toContain("network error");
    });

    it("populates vaultLockStates from vault_is_locked for each vault", async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "vault_list") return [MOCK_VAULT, MOCK_VAULT_2];
        if (cmd === "vault_is_locked") return true;
        return undefined;
      });

      await useVaultStore.getState().listVaults();

      const lockStates = useVaultStore.getState().vaultLockStates;
      expect(lockStates["vault-1"]).toBe(true);
      expect(lockStates["vault-2"]).toBe(true);
    });
  });

  // ── unlockVault ──

  describe("unlockVault", () => {
    it("sets vaultLocked to false on success", async () => {
      useVaultStore.setState({
        activeVaultId: "vault-1",
        vaultLockStates: { "vault-1": true },
      });
      mockInvoke.mockResolvedValueOnce(undefined); // vault_unlock
      mockInvoke.mockResolvedValueOnce([]);         // credential_list

      await useVaultStore.getState().unlockVault("vault-1", "master-pass");

      expect(mockInvoke).toHaveBeenCalledWith("vault_unlock", {
        vaultId: "vault-1",
        masterPassword: "master-pass",
      });
      expect(useVaultStore.getState().vaultLocked).toBe(false);
      expect(useVaultStore.getState().loading).toBe(false);
    });

    it("sets activeVaultId to the unlocked vault", async () => {
      useVaultStore.setState({ vaultLockStates: { "vault-1": true } });
      mockInvoke.mockResolvedValueOnce(undefined); // vault_unlock
      mockInvoke.mockResolvedValueOnce([]);         // credential_list

      await useVaultStore.getState().unlockVault("vault-1", "correct-pass");

      expect(useVaultStore.getState().activeVaultId).toBe("vault-1");
    });

    it("sets error and keeps vaultLocked true when unlock throws", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("wrong password"));

      await expect(
        useVaultStore.getState().unlockVault("vault-1", "bad-pass")
      ).rejects.toThrow();

      expect(useVaultStore.getState().vaultLocked).toBe(true);
      expect(useVaultStore.getState().error).toContain("wrong password");
      expect(useVaultStore.getState().loading).toBe(false);
    });
  });

  // ── lockAllVaults ──

  describe("lockAllVaults", () => {
    it("sets vaultLocked to true and clears credentials after lock all", async () => {
      useVaultStore.setState({
        vaults: [MOCK_VAULT],
        activeVaultId: "vault-1",
        vaultLockStates: { "vault-1": false },
        vaultLocked: false,
        credentials: [
          {
            id: "c1",
            name: "Test",
            credential_type: "password",
            username: "user",
            tags: [],
            created_at: "",
            updated_at: "",
          },
        ],
      });

      mockInvoke.mockResolvedValueOnce(undefined); // vault_lock_all

      await useVaultStore.getState().lockAllVaults();

      expect(useVaultStore.getState().vaultLocked).toBe(true);
      expect(useVaultStore.getState().credentials).toHaveLength(0);
    });

    it("marks all vaults as locked in vaultLockStates", async () => {
      useVaultStore.setState({
        vaults: [MOCK_VAULT, MOCK_VAULT_2],
        vaultLockStates: { "vault-1": false, "vault-2": false },
        vaultLocked: false,
      });

      mockInvoke.mockResolvedValueOnce(undefined);

      await useVaultStore.getState().lockAllVaults();

      const lockStates = useVaultStore.getState().vaultLockStates;
      expect(lockStates["vault-1"]).toBe(true);
      expect(lockStates["vault-2"]).toBe(true);
    });
  });

  // ── createVault ──

  describe("createVault", () => {
    it("calls invoke('vault_create') with correct arguments", async () => {
      mockInvoke.mockResolvedValueOnce(MOCK_VAULT); // vault_create

      await useVaultStore.getState().createVault("strongpass", "My Vault", true);

      expect(mockInvoke).toHaveBeenCalledWith("vault_create", {
        profileId: "default",
        masterPassword: "strongpass",
        name: "My Vault",
        isDefault: true,
      });
    });

    it("adds the new vault to the vaults list", async () => {
      mockInvoke.mockResolvedValueOnce(MOCK_VAULT);

      await useVaultStore.getState().createVault("strongpass");

      expect(useVaultStore.getState().vaults).toHaveLength(1);
      expect(useVaultStore.getState().vaults[0].id).toBe("vault-1");
    });

    it("sets vaultLocked to false after successful creation", async () => {
      mockInvoke.mockResolvedValueOnce(MOCK_VAULT);

      await useVaultStore.getState().createVault("strongpass");

      expect(useVaultStore.getState().vaultLocked).toBe(false);
    });

    it("sets error and rethrows when createVault fails", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("disk full"));

      await expect(
        useVaultStore.getState().createVault("pass")
      ).rejects.toThrow();

      expect(useVaultStore.getState().error).toContain("disk full");
      expect(useVaultStore.getState().loading).toBe(false);
    });
  });

  // ── deleteVault ──

  describe("deleteVault", () => {
    it("calls invoke('vault_delete') and removes vault from list", async () => {
      useVaultStore.setState({
        vaults: [MOCK_VAULT, MOCK_VAULT_2],
        activeVaultId: "vault-2",
        vaultLockStates: { "vault-1": true, "vault-2": false },
        vaultLocked: false,
      });

      mockInvoke.mockResolvedValueOnce(undefined); // vault_delete
      mockInvoke.mockResolvedValueOnce([]);         // credential_list (fetchCredentials)

      await useVaultStore.getState().deleteVault("vault-1", "pass");

      expect(mockInvoke).toHaveBeenCalledWith("vault_delete", {
        vaultId: "vault-1",
        password: "pass",
      });
      expect(useVaultStore.getState().vaults).toHaveLength(1);
      expect(useVaultStore.getState().vaults[0].id).toBe("vault-2");
    });

    it("sets error and rethrows when deleteVault fails", async () => {
      useVaultStore.setState({
        vaults: [MOCK_VAULT],
        activeVaultId: "vault-1",
        vaultLockStates: { "vault-1": false },
      });

      mockInvoke.mockRejectedValueOnce(new Error("wrong password"));

      await expect(
        useVaultStore.getState().deleteVault("vault-1", "bad-pass")
      ).rejects.toThrow();

      expect(useVaultStore.getState().error).toContain("wrong password");
    });
  });

  // ── addCredential ──

  describe("addCredential", () => {
    it("invokes credential_create and refreshes list", async () => {
      useVaultStore.setState({
        vaultLocked: false,
        activeVaultId: "vault-1",
        vaultLockStates: { "vault-1": false },
      });

      mockInvoke.mockResolvedValueOnce("new-cred-id"); // credential_create
      mockInvoke.mockResolvedValueOnce([               // credential_list (fetchCredentials)
        {
          id: "new-cred-id",
          name: "My Server",
          credential_type: "password",
          username: "admin",
          tags: [],
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ]);

      const id = await useVaultStore.getState().addCredential({
        name: "My Server",
        credential_type: "password",
        username: "admin",
        data: { password: "secret" },
      });

      expect(id).toBe("new-cred-id");
      expect(mockInvoke).toHaveBeenCalledWith("credential_create", {
        vaultId: "vault-1",
        request: {
          name: "My Server",
          credential_type: "password",
          username: "admin",
          data: { password: "secret" },
        },
      });
      expect(useVaultStore.getState().credentials).toHaveLength(1);
      expect(useVaultStore.getState().loading).toBe(false);
    });

    it("sets error on failure", async () => {
      useVaultStore.setState({
        vaultLocked: false,
        activeVaultId: "vault-1",
        vaultLockStates: { "vault-1": false },
      });
      mockInvoke.mockRejectedValueOnce(new Error("encryption failed"));

      await expect(
        useVaultStore.getState().addCredential({
          name: "Bad",
          credential_type: "password",
          data: {},
        })
      ).rejects.toThrow();

      expect(useVaultStore.getState().error).toContain("encryption failed");
    });
  });

  // ── deleteCredential ──

  describe("deleteCredential", () => {
    it("removes credential from local state", async () => {
      useVaultStore.setState({
        vaultLocked: false,
        activeVaultId: "vault-1",
        vaultLockStates: { "vault-1": false },
        credentials: [
          {
            id: "del-1",
            name: "ToDelete",
            credential_type: "password",
            username: null,
            tags: [],
            created_at: "",
            updated_at: "",
          },
        ],
      });

      mockInvoke.mockResolvedValueOnce(undefined); // credential_delete

      await useVaultStore.getState().deleteCredential("del-1");

      expect(useVaultStore.getState().credentials).toHaveLength(0);
    });
  });

  // ── markVaultLocked ──

  describe("markVaultLocked", () => {
    it("marks the vault as locked and updates vaultLocked", () => {
      useVaultStore.setState({
        vaults: [MOCK_VAULT],
        activeVaultId: "vault-1",
        vaultLockStates: { "vault-1": false },
        vaultLocked: false,
        credentials: [
          {
            id: "c1",
            name: "Cred",
            credential_type: "password",
            username: null,
            tags: [],
            created_at: "",
            updated_at: "",
          },
        ],
      });

      useVaultStore.getState().markVaultLocked("vault-1");

      expect(useVaultStore.getState().vaultLockStates["vault-1"]).toBe(true);
      expect(useVaultStore.getState().vaultLocked).toBe(true);
      expect(useVaultStore.getState().credentials).toHaveLength(0);
    });
  });

  // ── clearError ──

  describe("clearError", () => {
    it("resets error to null", () => {
      useVaultStore.setState({ error: "something broke" });
      useVaultStore.getState().clearError();
      expect(useVaultStore.getState().error).toBeNull();
    });
  });
});
