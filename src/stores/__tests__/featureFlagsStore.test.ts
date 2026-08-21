import { describe, it, expect, beforeEach, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useFeatureFlagsStore, getFlag, type FeatureFlags } from "@/stores/featureFlagsStore";

const mockInvoke = vi.mocked(invoke);

const DEFAULT_FLAGS: FeatureFlags = {
  sentry_crash_reporter: false,
  saml_sso: false,
  yubikey_hardware_fido2: false,
  nps_survey: false,
  aircrack_tools: false,
  web_relay: false,
  wasm_plugins: false,
};

function resetStore() {
  useFeatureFlagsStore.setState({ ...DEFAULT_FLAGS, loading: false });
}

describe("featureFlagsStore", () => {
  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  // ── Initial state ──

  describe("initial state", () => {
    it("has every flag disabled and loading false", () => {
      const state = useFeatureFlagsStore.getState();
      expect(state).toMatchObject(DEFAULT_FLAGS);
      expect(state.loading).toBe(false);
    });
  });

  // ── load ──

  describe("load", () => {
    it("sets loading true while the request is in flight", () => {
      let resolveInvoke!: (value: FeatureFlags) => void;
      mockInvoke.mockReturnValue(
        new Promise<FeatureFlags>((resolve) => {
          resolveInvoke = resolve;
        })
      );

      const promise = useFeatureFlagsStore.getState().load();

      expect(useFeatureFlagsStore.getState().loading).toBe(true);

      resolveInvoke(DEFAULT_FLAGS);
      return promise;
    });

    it("calls invoke('config_get_feature_flags') and applies the returned flags", async () => {
      mockInvoke.mockResolvedValue({ ...DEFAULT_FLAGS, wasm_plugins: true, web_relay: true });

      await useFeatureFlagsStore.getState().load();

      expect(mockInvoke).toHaveBeenCalledWith("config_get_feature_flags");
      const state = useFeatureFlagsStore.getState();
      expect(state.wasm_plugins).toBe(true);
      expect(state.web_relay).toBe(true);
      expect(state.loading).toBe(false);
    });

    it("sets loading false and keeps existing flags when invoke rejects", async () => {
      useFeatureFlagsStore.setState({ aircrack_tools: true });
      mockInvoke.mockRejectedValue(new Error("backend unavailable"));

      await useFeatureFlagsStore.getState().load();

      const state = useFeatureFlagsStore.getState();
      expect(state.loading).toBe(false);
      // Failure is swallowed; whatever was in state before the call is untouched.
      expect(state.aircrack_tools).toBe(true);
    });
  });

  // ── setFlag ──

  describe("setFlag", () => {
    it("calls invoke('config_set_feature_flag') with the flag name and desired value", async () => {
      mockInvoke.mockResolvedValue({ ...DEFAULT_FLAGS, saml_sso: true });

      await useFeatureFlagsStore.getState().setFlag("saml_sso", true);

      expect(mockInvoke).toHaveBeenCalledWith("config_set_feature_flag", {
        flag: "saml_sso",
        enabled: true,
      });
      expect(useFeatureFlagsStore.getState().saml_sso).toBe(true);
    });

    it("replaces state with the full flags object returned by the backend", async () => {
      mockInvoke.mockResolvedValue({
        ...DEFAULT_FLAGS,
        saml_sso: true,
        nps_survey: true,
      });

      await useFeatureFlagsStore.getState().setFlag("saml_sso", true);

      const state = useFeatureFlagsStore.getState();
      expect(state.saml_sso).toBe(true);
      expect(state.nps_survey).toBe(true);
      expect(state.sentry_crash_reporter).toBe(false);
    });

    it("logs an error and leaves flags unchanged when invoke rejects", async () => {
      const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
      mockInvoke.mockRejectedValue(new Error("permission denied"));

      await useFeatureFlagsStore.getState().setFlag("yubikey_hardware_fido2", true);

      expect(useFeatureFlagsStore.getState().yubikey_hardware_fido2).toBe(false);
      expect(consoleError).toHaveBeenCalledWith(
        "Failed to set feature flag:",
        expect.any(Error)
      );
      consoleError.mockRestore();
    });
  });

  // ── getFlag ──

  describe("getFlag", () => {
    it("reads the current value of a flag directly from the store", () => {
      expect(getFlag("web_relay")).toBe(false);

      useFeatureFlagsStore.setState({ web_relay: true });

      expect(getFlag("web_relay")).toBe(true);
    });
  });
});
