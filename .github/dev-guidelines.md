# CrossTerm — Development Guidelines

## Project Overview

CrossTerm is a cross-platform terminal emulator and remote access suite built with **Tauri 2.x** (Rust backend + React/TypeScript frontend). Reference specifications:

- Product spec: `SPEC-CROSSTERM-001.md`
- Architecture: `docs/ARCHITECTURE.md`
- Design system: `docs/DESIGN.md`
- Coding standards: `docs/CODING-STANDARDS.md`
- QA & testing: `docs/QA.md`

## Architecture Rules

- **Tauri IPC boundary**: Frontend communicates with Rust backend ONLY via `invoke()` (commands) and `listen()` (events). No direct filesystem, process, or network access from the frontend.
- **Command naming**: `{module}_{verb}` — e.g., `vault_unlock`, `ssh_connect`, `terminal_write`.
- **Event naming**: `{module}:{event-name}` with colon separator — e.g., `terminal:output`, `ssh:disconnected`.
- **State management**: Each Rust module has ONE state struct registered via `tauri::Builder::manage()`. Frontend has four Zustand stores: `appStore` (UI), `sessionStore` (sessions/tabs), `terminalStore` (terminal instances), `vaultStore` (credentials).
- **Error types**: One `thiserror` enum per Rust module. All implement `Serialize` by delegating to `Display`. All commands return `Result<T, ModuleError>`.

## Frontend Standards

- **React 18** with **function components only** and hooks. No class components.
- **TypeScript strict mode**. No `any` — use `unknown` with type guards.
- **ALL types, enums, and interfaces** live in `src/types/index.ts`. Only exception: component-local props interfaces.
- **Zustand stores**: Always use selector pattern — `useAppStore((s) => s.field)`, NEVER `useAppStore()`.
- **Imports**: Use `@/` path alias for all internal imports. Use `import type` for type-only imports.
- **Styling**: TailwindCSS utility classes with design token mapping. Use `bg-surface-primary`, `text-text-secondary`, etc. NEVER hardcode color values. Use `clsx()` for conditional classes.
- **i18n**: All user-facing strings via `useTranslation()` hook and `t()` function. No hardcoded strings in JSX.
- **Icons**: Lucide React. Use consistent sizes: 20px sidebar rail, 16px actions, 14px inline, 11px status bar.

## Rust Backend Standards

- **Edition 2021**, MSRV 1.77.2.
- **Module structure**: Each module in `src-tauri/src/{module}/mod.rs` follows: Error → Types → Helpers → State struct → Tauri commands.
- **Section markers**: Use `// ── Section Name ──` pattern for visual separation.
- **Error handling**: Use `?` operator. Never `.unwrap()` in command paths. `.unwrap()` only for `Mutex::lock()`.
- **Serde**: `#[serde(rename_all = "snake_case")]` on enums. `#[serde(tag = "type")]` for tagged unions. `#[serde(skip_serializing)]` on sensitive fields.
- **Security**: Credentials encrypted with AES-256-GCM. Key material in `Zeroizing<Vec<u8>>`. Never log secrets. UUID v4 for all identifiers.
- **Concurrency**: `Mutex` for write-heavy state, `RwLock` for read-heavy. Tokio async for SSH/network. `std::thread` for PTY I/O.

## Design System

- **Token-based theming**: CSS custom properties → Tailwind config → utility classes. Three-layer indirection.
- **Six-region layout**: TitleBar (A), TabBar (B), Sidebar (C), SessionCanvas (D), BottomPanel (E), StatusBar (F).
- **4px spacing grid**: All spacing in multiples of 4px.
- **Animation timing**: Use CSS variables (`--duration-micro` through `--duration-long`). Respect `prefers-reduced-motion`.
- **Empty states**: Always show icon + message + optional action button. Never blank panels.

## Key Patterns

- **Discriminated unions** with `type` field for polymorphic data (Credential, SplitPane, SshAuth, PortForward).
- **Optimistic UI updates** in vaultStore — update local state immediately, revert on error.
- **Observer pattern** for terminal/SSH output — backend `emit()` → frontend `listen()`.
- **Singleton managed state** per Rust module via `tauri::manage()`.

## Testing

- Rust: `#[cfg(test)]` inline modules. ≥80% line coverage.
- Frontend: Vitest for stores. ≥70% coverage on stores/utilities.
- E2E: Playwright. 20 critical user journeys.
- Security: `cargo audit`, `npm audit`, `cargo clippy -D warnings` all clean.
- Validate tests against specs and written code before merging. In case of drift, resolve the drift by validating against the spec.
- Do not assume signatures , API's or other details that are not explicitly defined in the spec. If the spec is missing details, add them to the spec and validate against those.

## File Conventions

- Components: PascalCase `.tsx` in PascalCase directories under `src/components/`.
- Stores: camelCase `xxxStore.ts` in `src/stores/`.
- Rust modules: `mod.rs` in `src-tauri/src/{module}/`.
- Themes: lowercase `.json` in `src/themes/`.
- i18n: lowercase `.json` in `src/i18n/`.
