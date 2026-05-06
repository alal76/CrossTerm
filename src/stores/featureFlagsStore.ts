import { create } from "zustand";
import { persist } from "zustand/middleware";
import { invoke } from "@tauri-apps/api/core";

export interface FeatureFlags {
  /** Sentry crash reporting — requires SENTRY_DSN env var. See docs/V1.2_MANUAL_STEPS.md §1. */
  sentry_crash_reporter: boolean;
  /** SAML 2.0 SP — requires IdP metadata URL. See docs/V1.2_MANUAL_STEPS.md §2. */
  saml_sso: boolean;
  /** Hardware YubiKey/FIDO2 unlock — requires physical key. See docs/V1.2_MANUAL_STEPS.md §8. */
  yubikey_hardware_fido2: boolean;
  /** NPS survey — requires Delighted/Typeform account. See docs/V1.2_MANUAL_STEPS.md §12. */
  nps_survey: boolean;
  /** aircrack-ng WiFi audit tools — security-sensitive; also requires disclaimer acceptance. */
  aircrack_tools: boolean;
  /** WebSocket thin-client relay — exposes a local port. */
  web_relay: boolean;
  /** WASM plugin runtime via wasmtime. */
  wasm_plugins: boolean;
}

const DEFAULT_FLAGS: FeatureFlags = {
  sentry_crash_reporter: false,
  saml_sso: false,
  yubikey_hardware_fido2: false,
  nps_survey: false,
  aircrack_tools: false,
  web_relay: false,
  wasm_plugins: false,
};

interface FeatureFlagsState extends FeatureFlags {
  loading: boolean;
  load: () => Promise<void>;
  setFlag: (flag: keyof FeatureFlags, enabled: boolean) => Promise<void>;
}

export const useFeatureFlagsStore = create<FeatureFlagsState>()(
  persist(
    (set) => ({
      ...DEFAULT_FLAGS,
      loading: false,

      load: async () => {
        set({ loading: true });
        try {
          const flags = await invoke<FeatureFlags>("config_get_feature_flags");
          set({ ...flags, loading: false });
        } catch {
          set({ loading: false });
        }
      },

      setFlag: async (flag: keyof FeatureFlags, enabled: boolean) => {
        try {
          const updated = await invoke<FeatureFlags>("config_set_feature_flag", {
            flag,
            enabled,
          });
          set({ ...updated });
        } catch (err) {
          console.error("Failed to set feature flag:", err);
        }
      },
    }),
    {
      name: "crossterm-feature-flags",
      partialize: (state) => {
        const { loading: _l, load: _lo, setFlag: _sf, ...flags } = state;
        return flags;
      },
    }
  )
);

export function getFlag(flag: keyof FeatureFlags): boolean {
  return useFeatureFlagsStore.getState()[flag] as boolean;
}
