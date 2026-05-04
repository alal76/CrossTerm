/**
 * SsoButton — "Sign in with {provider}" button for OIDC SSO vault unlock.
 *
 * Usage in VaultUnlock (or any other parent):
 *
 *   <SsoButton
 *     providerName="Google"
 *     onSuccess={(profile) => handleSsoUnlock(profile)}
 *     onError={(msg) => showToast(msg)}
 *   />
 *
 * The component triggers the full Authorization Code + PKCE flow implemented
 * in `src-tauri/src/auth/mod.rs` via the `auth_oidc_begin` Tauri command.
 * It expects the OIDC configuration for `providerName` to have been previously
 * saved via `auth_save_oidc_config` so the backend can look it up.
 *
 * SsoConfigForm (not exported as default — used by Settings only) provides
 * the UI for adding/editing OIDC provider configurations.
 */

import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Globe,
  Loader2,
  ShieldCheck,
  Trash2,
  Plus,
  Save,
  AlertCircle,
} from "lucide-react";

// ── Shared types ────────────────────────────────────────────────────────

/**
 * The subset of OIDC ID-token claims that CrossTerm surfaces to the
 * application layer.  Mirrors `OidcProfile` in `src-tauri/src/auth/mod.rs`.
 */
export interface OidcProfile {
  /** Subject — unique user identifier from the IdP. */
  sub: string;
  email?: string;
  name?: string;
  /** URL of the user's profile picture, if provided by the IdP. */
  picture?: string;
}

/** Full result returned by `auth_oidc_begin`. */
interface OidcFlowResult {
  profile: OidcProfile;
  access_token: string;
  id_token: string;
  callback_port: number;
}

/**
 * Provider configuration passed to `auth_save_oidc_config`.
 * Mirrors `OidcConfig` in `src-tauri/src/auth/mod.rs`.
 */
export interface OidcConfig {
  provider_name: string;
  client_id: string;
  authorization_endpoint: string;
  token_endpoint: string;
  userinfo_endpoint?: string;
  /** Defaults to ["openid", "email", "profile"] on the backend if left empty. */
  scopes: string[];
}

// ── SsoButton ────────────────────────────────────────────────────────────

export interface SsoButtonProps {
  /** Display name used to label the button and look up the provider config. */
  providerName: string;
  /** Called with the parsed OIDC profile once the flow completes successfully. */
  onSuccess: (profile: OidcProfile) => void;
  /** Called with a human-readable error message if any step fails. */
  onError: (error: string) => void;
  disabled?: boolean;
}

/**
 * Primary export.  Renders a "Sign in with {providerName}" button styled to
 * match CrossTerm's surface/border design language.  While the async OIDC
 * flow is in progress (browser open, waiting for callback, token exchange) the
 * button shows a spinner and is non-interactive.
 */
export default function SsoButton({
  providerName,
  onSuccess,
  onError,
  disabled = false,
}: SsoButtonProps) {
  const [loading, setLoading] = useState(false);

  const handleClick = useCallback(async () => {
    if (loading || disabled) return;
    setLoading(true);
    try {
      // The backend looks up the stored OidcConfig by provider_name, so we only
      // need to pass the name here — the full config was persisted earlier via
      // auth_save_oidc_config.
      const result = await invoke<OidcFlowResult>("auth_oidc_begin", {
        // The Tauri command expects a full OidcConfig struct.  We send a minimal
        // sentinel that the backend resolves from its AuthState.  If no config
        // has been stored the backend will return an error which surfaces via
        // onError below.
        config: {
          provider_name: providerName,
          client_id: "",
          authorization_endpoint: "",
          token_endpoint: "",
          scopes: [],
        },
      });
      onSuccess(result.profile);
    } catch (err) {
      onError(
        typeof err === "string"
          ? err
          : err instanceof Error
            ? err.message
            : "OIDC authentication failed",
      );
    } finally {
      setLoading(false);
    }
  }, [loading, disabled, providerName, onSuccess, onError]);

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={disabled || loading}
      className={clsx(
        // Layout
        "w-full flex items-center justify-center gap-2.5 px-4 py-2.5 rounded-lg",
        // Typography
        "text-sm font-medium",
        // Border + background — matches the biometric/FIDO2 buttons in VaultUnlock
        "border border-border-default transition-colors duration-[var(--duration-short)]",
        disabled || loading
          ? "bg-interactive-disabled text-text-disabled cursor-not-allowed border-border-default"
          : "bg-surface-secondary hover:bg-surface-tertiary text-text-primary",
      )}
      aria-busy={loading}
    >
      {loading ? (
        <Loader2 size={16} className="animate-spin shrink-0" />
      ) : (
        <Globe size={16} className="shrink-0 text-text-secondary" />
      )}
      <span>
        {loading ? "Signing in…" : `Sign in with ${providerName}`}
      </span>
    </button>
  );
}

// ── SsoConfigForm ─────────────────────────────────────────────────────────

interface SsoConfigFormProps {
  /** Called after the user saves a new or updated config. */
  onSaved?: (config: OidcConfig) => void;
  /** Pre-fill the form for editing an existing provider. */
  initialConfig?: Partial<OidcConfig>;
}

/**
 * Form for adding or editing an OIDC provider configuration.
 * NOT the default export — consumed only by the Settings panel.
 *
 * On submit it calls `auth_save_oidc_config` which stores the config in
 * `AuthState` so `auth_oidc_begin` can retrieve it later.
 */
export function SsoConfigForm({ onSaved, initialConfig }: SsoConfigFormProps) {
  const [providerName, setProviderName] = useState(
    initialConfig?.provider_name ?? "",
  );
  const [clientId, setClientId] = useState(initialConfig?.client_id ?? "");
  const [authEndpoint, setAuthEndpoint] = useState(
    initialConfig?.authorization_endpoint ?? "",
  );
  const [tokenEndpoint, setTokenEndpoint] = useState(
    initialConfig?.token_endpoint ?? "",
  );
  const [userinfoEndpoint, setUserinfoEndpoint] = useState(
    initialConfig?.userinfo_endpoint ?? "",
  );
  const [scopesRaw, setScopesRaw] = useState(
    initialConfig?.scopes?.join(" ") ?? "openid email profile",
  );

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError(null);
      setSaved(false);

      if (!providerName.trim()) {
        setError("Provider name is required.");
        return;
      }
      if (!clientId.trim()) {
        setError("Client ID is required.");
        return;
      }
      if (!authEndpoint.trim()) {
        setError("Authorization endpoint is required.");
        return;
      }
      if (!tokenEndpoint.trim()) {
        setError("Token endpoint is required.");
        return;
      }

      const config: OidcConfig = {
        provider_name: providerName.trim(),
        client_id: clientId.trim(),
        authorization_endpoint: authEndpoint.trim(),
        token_endpoint: tokenEndpoint.trim(),
        userinfo_endpoint: userinfoEndpoint.trim() || undefined,
        scopes: scopesRaw
          .split(/\s+/)
          .map((s) => s.trim())
          .filter(Boolean),
      };

      setSaving(true);
      try {
        await invoke("auth_save_oidc_config", { config });
        setSaved(true);
        onSaved?.(config);
      } catch (err) {
        setError(
          typeof err === "string"
            ? err
            : err instanceof Error
              ? err.message
              : "Failed to save OIDC configuration",
        );
      } finally {
        setSaving(false);
      }
    },
    [
      providerName,
      clientId,
      authEndpoint,
      tokenEndpoint,
      userinfoEndpoint,
      scopesRaw,
      onSaved,
    ],
  );

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4">
      {/* Header */}
      <div className="flex items-center gap-2">
        <ShieldCheck size={16} className="text-accent-primary shrink-0" />
        <span className="text-sm font-semibold text-text-primary">
          OIDC Provider Configuration
        </span>
      </div>

      {/* Fields */}
      <div className="flex flex-col gap-3">
        <Field
          label="Provider name"
          hint="Shown on the Sign-in button (e.g. Google, Okta, Azure AD)"
        >
          <input
            type="text"
            value={providerName}
            onChange={(e) => setProviderName(e.target.value)}
            placeholder="e.g. Okta"
            className={inputCls}
          />
        </Field>

        <Field label="Client ID" hint="The OAuth 2.0 client_id from your IdP">
          <input
            type="text"
            value={clientId}
            onChange={(e) => setClientId(e.target.value)}
            placeholder="0oa1b2c3d4e5f6g7h8i9"
            className={inputCls}
          />
        </Field>

        <Field
          label="Authorization endpoint"
          hint="IdP /authorize URL"
        >
          <input
            type="url"
            value={authEndpoint}
            onChange={(e) => setAuthEndpoint(e.target.value)}
            placeholder="https://your-idp.example.com/oauth2/authorize"
            className={inputCls}
          />
        </Field>

        <Field
          label="Token endpoint"
          hint="IdP /token URL (used to exchange the authorization code)"
        >
          <input
            type="url"
            value={tokenEndpoint}
            onChange={(e) => setTokenEndpoint(e.target.value)}
            placeholder="https://your-idp.example.com/oauth2/token"
            className={inputCls}
          />
        </Field>

        <Field
          label="UserInfo endpoint (optional)"
          hint="If provided, CrossTerm can fetch additional profile claims"
        >
          <input
            type="url"
            value={userinfoEndpoint}
            onChange={(e) => setUserinfoEndpoint(e.target.value)}
            placeholder="https://your-idp.example.com/oauth2/userinfo"
            className={inputCls}
          />
        </Field>

        <Field
          label="Scopes"
          hint='Space-separated OAuth scopes. Defaults to "openid email profile"'
        >
          <input
            type="text"
            value={scopesRaw}
            onChange={(e) => setScopesRaw(e.target.value)}
            placeholder="openid email profile"
            className={inputCls}
          />
        </Field>
      </div>

      {/* Feedback */}
      {error && (
        <p className="flex items-center gap-1.5 text-xs text-status-disconnected">
          <AlertCircle size={13} className="shrink-0" />
          {error}
        </p>
      )}
      {saved && !error && (
        <p className="text-xs text-status-connected">
          Provider configuration saved.
        </p>
      )}

      {/* Submit */}
      <button
        type="submit"
        disabled={saving}
        className={clsx(
          "flex items-center justify-center gap-2 w-full py-2.5 rounded-lg text-sm font-medium",
          "transition-colors duration-[var(--duration-short)]",
          saving
            ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
            : "bg-interactive-default hover:bg-interactive-hover text-text-primary",
        )}
      >
        {saving ? (
          <Loader2 size={15} className="animate-spin" />
        ) : (
          <Save size={15} />
        )}
        {saving ? "Saving…" : "Save provider"}
      </button>
    </form>
  );
}

// ── SsoProviderList ───────────────────────────────────────────────────────

interface SsoProviderListProps {
  configs: OidcConfig[];
  onDelete: (providerName: string) => void;
  onAdd: () => void;
  deleting?: string | null;
}

/**
 * Read-only list of configured OIDC providers with delete buttons.
 * Designed for use inside the Settings panel alongside SsoConfigForm.
 * NOT the default export.
 */
export function SsoProviderList({
  configs,
  onDelete,
  onAdd,
  deleting,
}: SsoProviderListProps) {
  return (
    <div className="flex flex-col gap-2">
      {configs.length === 0 && (
        <p className="text-xs text-text-disabled px-1">
          No OIDC providers configured.
        </p>
      )}

      {configs.map((c) => (
        <div
          key={c.provider_name}
          className="flex items-center gap-3 px-3 py-2 rounded-lg border border-border-default bg-surface-secondary"
        >
          <Globe size={14} className="text-text-secondary shrink-0" />
          <div className="flex-1 min-w-0">
            <p className="text-sm text-text-primary truncate">
              {c.provider_name}
            </p>
            <p className="text-[10px] text-text-disabled truncate">
              {c.client_id}
            </p>
          </div>
          <button
            type="button"
            disabled={deleting === c.provider_name}
            onClick={() => onDelete(c.provider_name)}
            title={`Remove ${c.provider_name}`}
            className="shrink-0 p-1 rounded text-text-disabled hover:text-status-disconnected hover:bg-status-disconnected/10 transition-colors"
          >
            {deleting === c.provider_name ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Trash2 size={13} />
            )}
          </button>
        </div>
      ))}

      <button
        type="button"
        onClick={onAdd}
        className="flex items-center gap-1.5 text-xs text-text-secondary hover:text-accent-primary transition-colors mt-1"
      >
        <Plus size={13} />
        Add provider
      </button>
    </div>
  );
}

// ── Internal helpers ──────────────────────────────────────────────────────

const inputCls = clsx(
  "w-full px-3 py-2 rounded-lg text-sm",
  "bg-surface-secondary border border-border-default outline-none",
  "text-text-primary placeholder:text-text-disabled",
  "focus:border-border-focus transition-colors duration-[var(--duration-short)]",
);

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-xs font-medium text-text-secondary">{label}</label>
      {children}
      {hint && <p className="text-[10px] text-text-disabled px-0.5">{hint}</p>}
    </div>
  );
}
