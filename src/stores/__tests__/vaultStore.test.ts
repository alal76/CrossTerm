import { describe, it, expect, beforeEach, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useVaultStore } from "@/stores/vaultStore";

const mockInvoke = vi.mocked(invoke);

function resetStore() {
  useVaultStore.setState({
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

  // ── Unlock ──

  describe("unlockVault", () => {
    it("sets vaultLocked to false on success", async () => {
      mockInvoke.mockResolvedValueOnce(undefined); // vault_unlock
      mockInvoke.mockResolvedValueOnce([]);         // credential_list

      await useVaultStore.getState().unlockVault("master-pass");

      expect(mockInvoke).toHaveBeenCalledWith("vault_unlock", {
        profileId: "default",
        masterPassword: "master-pass",
      });
      expect(useVaultStore.getState().vaultLocked).toBe(false);
      expect(useVaultStore.getState().loading).toBe(false);
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("wrong password"));

      await expect(
        useVaultStore.getState().unlockVault("bad-pass")
      ).rejects.toThrow();

      expect(useVaultStore.getState().vaultLocked).toBe(true);
      expect(useVaultStore.getState().error).toContain("wrong password");
      expect(useVaultStore.getState().loading).toBe(false);
    });
  });

  // ── Lock ──

  describe("lockVault", () => {
    it("sets vaultLocked to true and clears credentials", async () => {
      // Start unlocked
      useVaultStore.setState({
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

      mockInvoke.mockResolvedValueOnce(undefined); // vault_lock

      await useVaultStore.getState().lockVault();

      expect(useVaultStore.getState().vaultLocked).toBe(true);
      expect(useVaultStore.getState().credentials).toHaveLength(0);
    });
  });

  // ── Add Credential ──

  describe("addCredential", () => {
    it("invokes credential_create and refreshes list", async () => {
      useVaultStore.setState({ vaultLocked: false });

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
      useVaultStore.setState({ vaultLocked: false });
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

  // ── Delete Credential ──

  describe("deleteCredential", () => {
    it("removes credential from local state", async () => {
      useVaultStore.setState({
        vaultLocked: false,
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

  // ── Clear Error ──

  describe("clearError", () => {
    it("resets error to null", () => {
      useVaultStore.setState({ error: "something broke" });
      useVaultStore.getState().clearError();
      expect(useVaultStore.getState().error).toBeNull();
    });
  });
});
