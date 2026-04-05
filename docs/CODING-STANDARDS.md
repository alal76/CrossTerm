# CrossTerm — Coding Standards

| Field          | Value                                |
|----------------|--------------------------------------|
| Spec ID        | STANDARDS-CROSSTERM-001              |
| Version        | 1.0                                  |
| Status         | Active                               |
| Last Updated   | 2026-04-05                           |
| Parent Spec    | SPEC-CROSSTERM-001, ARCH-CROSSTERM-001 |

---

## 1. General Principles

1. **Spec-driven development** — All features trace back to SPEC-CROSSTERM-001. Reference the spec section (e.g., "per §6.2") in PR descriptions and commit messages when implementing spec requirements.
2. **Least surprise** — Follow established patterns in the codebase. New code should look like existing code.
3. **No over-engineering** — Don't add abstractions, helpers, or error handling for scenarios that can't happen. Only validate at system boundaries (IPC commands, user input).
4. **Security by default** — Credentials never in plaintext. Secrets zeroized on drop. No sensitive data in logs.

---

## 2. TypeScript / React Standards

### 2.1 Language & Compiler

- **TypeScript**: Strict mode enabled (`"strict": true` in `tsconfig.json`).
- **Target**: ES2021, bundled by Vite.
- **Path aliases**: Use `@/` prefix for imports from `src/` (configured in `tsconfig.json`).
- **No `any`**: Avoid `any`. Use `unknown` with type guards, or `Record<string, unknown>` for dynamic objects.

### 2.2 File & Directory Naming

| Item               | Convention                   | Example                      |
|--------------------|------------------------------|------------------------------|
| Component files    | PascalCase `.tsx`            | `TerminalView.tsx`           |
| Store files        | camelCase `Store.ts`         | `appStore.ts`                |
| Type files         | camelCase `.ts`              | `index.ts` (in `types/`)     |
| i18n locale files  | lowercase `.json`            | `en.json`                    |
| Theme files        | lowercase `.json`            | `dark.json`                  |
| Directories        | PascalCase (components)      | `Terminal/`, `SessionTree/`  |
| Directories        | camelCase (non-components)   | `stores/`, `themes/`, `i18n/`|

### 2.3 Component Conventions

#### Structure

```tsx
// 1. Imports (React, libraries, stores, types, CSS)
import { useEffect, useRef } from "react";
import clsx from "clsx";
import { SomeIcon } from "lucide-react";
import { useAppStore } from "@/stores/appStore";
import { SomeType } from "@/types";

// 2. Types/interfaces (if component-local)
interface MyComponentProps {
  readonly value: string;   // Use 'readonly' for props
  onAction?: () => void;
}

// 3. Component (default export for page/feature components)
export default function MyComponent({ value, onAction }: MyComponentProps) {
  // Hooks first
  const { t } = useTranslation();
  const someState = useAppStore((s) => s.someState);

  // Derived state
  const computed = useMemo(() => /* ... */, [dep]);

  // Effects
  useEffect(() => { /* ... */ }, [dep]);

  // Handlers
  const handleClick = useCallback(() => { /* ... */ }, [dep]);

  // Render
  return <div>...</div>;
}
```

#### Rules

- **Function components only** — No class components.
- **Named function declarations** — `function MyComponent()` not `const MyComponent = () =>`.
- **Default export** for primary component per file. Named exports for sub-components or utilities within the same file.
- **Props interface** at the top of the file, not inline. Use `readonly` for immutable props.
- **No prop drilling** — Use Zustand store selectors. Components subscribe to the state they need.
- **Selector pattern** — Always use selector functions: `useAppStore((s) => s.field)`, never `useAppStore()` (subscribes to entire store).

### 2.4 State Management (Zustand)

#### Store Definition Pattern

```typescript
import { create } from "zustand";
import { SomeEnum } from "@/types";
import type { SomeType } from "@/types";

interface MyState {
  // State fields
  items: SomeType[];
  activeId: string | null;

  // Actions (imperative verbs)
  addItem: (item: SomeType) => void;
  removeItem: (id: string) => void;
  setActive: (id: string) => void;
}

export const useMyStore = create<MyState>((set, get) => ({
  items: [],
  activeId: null,

  addItem: (item) => set((state) => ({ items: [...state.items, item] })),
  removeItem: (id) => set((state) => ({
    items: state.items.filter((i) => i.id !== id),
  })),
  setActive: (id) => set({ activeId: id }),
}));
```

#### Rules

- **One store per domain** — `appStore` (UI), `sessionStore` (sessions), `terminalStore` (terminals), `vaultStore` (vault).
- **Immutable updates** — Always return new objects/arrays in `set()`. Never mutate state.
- **Import separation** — Use `import type` for type-only imports. Use value imports for enums.
- **Async actions** — For IPC calls, use async actions that `set({ loading: true })` before and `set({ loading: false })` after, with `try/catch` that sets `error`.
- **Optimistic updates** — Update local state immediately, revert on error.

### 2.5 Type System

#### Enums

All enums are **string enums** with lowercase snake_case values:

```typescript
export enum SessionType {
  SSH = "ssh",
  SFTP = "sftp",
  LocalShell = "local_shell",
}
```

**Rationale**: String enums serialize directly to JSON, match Rust `#[serde(rename_all = "snake_case")]`, and are human-readable in debug output.

#### Discriminated Unions

Polymorphic types use a `type` discriminant:

```typescript
export interface SplitPaneLeaf {
  type: "leaf";
  tabId: string;
}
export interface SplitPaneContainer {
  type: "container";
  direction: SplitDirection;
  children: SplitPane[];
  sizes: number[];
}
export type SplitPane = SplitPaneLeaf | SplitPaneContainer;
```

#### Type Location

- **ALL types, interfaces, and enums** live in `src/types/index.ts`.
- Component-local types (props interfaces) are the only exception — they live in the component file.
- Never scatter domain types across multiple files.

### 2.6 Styling (TailwindCSS)

#### Rules

1. **Token classes only** — Use `bg-surface-primary`, not `bg-[#1a1b26]`. No arbitrary color values.
2. **Utility-first** — Define styles inline via className. No separate CSS files per component.
3. **`clsx` for conditionals** — Use `clsx()` for conditional class application:
   ```tsx
   className={clsx(
     "base-classes",
     isActive ? "active-classes" : "inactive-classes"
   )}
   ```
4. **Spacing tokens** — Use the `space-*` tokens or Tailwind's built-in spacing scale (multiples of 4px).
5. **No `!important`** — If specificity conflicts arise, restructure the class order instead.
6. **Responsive design** — Use the breakpoint system from Design spec §3.3. Use `useAppStore` `windowWidth` for JS-driven breakpoints.

### 2.7 Imports

#### Order

1. React / React hooks
2. Third-party libraries (`clsx`, `lucide-react`, `i18next`, etc.)
3. Tauri API imports (`@tauri-apps/api/*`)
4. Internal stores (`@/stores/*`)
5. Internal types (`@/types`)
6. Internal components (`@/components/*`)
7. CSS imports

#### Rules

- Use `@/` path alias for all internal imports. Never use relative paths beyond the current directory.
- Use `import type { ... }` for type-only imports (enforced by TypeScript's `isolatedModules`).
- Prefer named imports. Default imports only for components (one per file) and i18n module.

### 2.8 Internationalization

- **Every user-facing string** must come from `t()`. No hardcoded strings in JSX.
- Keys follow dot-notation hierarchy: `"domain.key"` (e.g., `"sidebar.sessions"`, `"vault.unlock"`).
- Interpolation uses `{{ variable }}` syntax: `t("emptyCanvas.hint", { shortcut: "Ctrl+T" })`.
- Numbers, dates, and file sizes use `Intl.*` APIs with the active locale.

---

## 3. Rust Standards

### 3.1 Language & Toolchain

- **Edition**: 2021.
- **MSRV**: 1.77.2 (specified in `Cargo.toml`).
- **Clippy**: Run at `warn` level minimum. Target `deny` level for CI.
- **Formatting**: `cargo fmt` with default rustfmt config.

### 3.2 Module Organization

```
src-tauri/src/
├── main.rs           # Entry point (just calls lib::run())
├── lib.rs            # Tauri setup, plugin registration, command registry
├── {module}/
│   └── mod.rs        # Module implementation
```

**Each module follows this internal structure:**

```rust
// 1. Error type (thiserror)
// 2. Types (structs, enums) — serde-annotated
// 3. Internal helpers
// 4. State struct (with constructor)
// 5. Tauri commands (#[tauri::command])
```

### 3.3 Error Handling Pattern

Every module defines its own error enum using `thiserror`:

```rust
#[derive(Debug, Error)]
pub enum ModuleError {
    #[error("Specific error: {0}")]
    SpecificVariant(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Serialize as display string for Tauri IPC
impl Serialize for ModuleError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
```

**Rules:**
- Every Tauri command returns `Result<T, ModuleError>`.
- Error types impl `Serialize` by delegating to `Display`.
- Use `#[from]` for automatic conversion from common error types (`std::io::Error`, `serde_json::Error`, `rusqlite::Error`).
- Never use `.unwrap()` in command paths. Use `?` operator.
- `.unwrap()` is acceptable only for `Mutex::lock()` where poisoning is unrecoverable.

### 3.4 State Management

```rust
pub struct MyState {
    inner: Mutex<HashMap<String, MySession>>,  // or RwLock for read-heavy
}

impl MyState {
    pub fn new() -> Self {
        Self { inner: Mutex::new(HashMap::new()) }
    }
}
```

**Rules:**
- One state struct per module, registered via `tauri::Builder::manage()`.
- Use `Mutex` for write-heavy state. Use `RwLock` for read-heavy state.
- For async modules (SSH), use `tokio::sync::Mutex` and `tokio::sync::RwLock`.
- State structs are never `Clone` — they hold interior mutability.

### 3.5 Tauri Command Conventions

```rust
#[tauri::command]
pub async fn module_action(
    app_handle: AppHandle,              // If events needed
    state: tauri::State<'_, MyState>,   // Module state
    param1: String,                     // Required params
    param2: Option<u32>,                // Optional params
) -> Result<ReturnType, ModuleError> {
    // Implementation
}
```

**Naming**: `{module}_{verb}` or `{module}_{noun}_{verb}`.

Examples: `vault_unlock`, `credential_create`, `ssh_connect`, `terminal_write`, `session_search`.

### 3.6 Serde Conventions

- Use `#[serde(rename_all = "snake_case")]` on all enums.
- Use `#[serde(tag = "type", rename_all = "snake_case")]` on tagged unions.
- Use `#[serde(skip_serializing)]` on fields that must never cross the IPC boundary (encrypted data, nonces).
- Request types: `FooCreateRequest`, `FooUpdateRequest` — separate from the entity type.
- Response types: Use the entity type directly, or a summary type that excludes secrets.

### 3.7 Security Coding Rules

1. **Credential data**: Always encrypted with AES-256-GCM before storage. Use `Zeroizing<Vec<u8>>` for key material.
2. **Password handling**: Wrap in `Zeroizing::new()` immediately upon receipt. Never clone passwords into non-zeroizing containers.
3. **Skip serialization**: Apply `#[serde(skip_serializing)]` to `encrypted_data`, `nonce`, and any raw secret field.
4. **Audit trail**: Call `audit::append_event()` for security-relevant operations (vault lock/unlock, credential CRUD, session connect/disconnect).
5. **No logging secrets**: Never `log::info!()` or `dbg!()` passwords, keys, or tokens.
6. **UUID generation**: Use `uuid::Uuid::new_v4()` for all identifiers. Never use sequential IDs.

### 3.8 Async Patterns

- SSH and network operations use **tokio** async runtime.
- PTY I/O uses **std::thread** (blocking I/O, not async-friendly).
- Use `mpsc::channel` to bridge between async tasks and command handlers.
- Never block the tokio runtime with synchronous I/O — spawn blocking tasks on `std::thread` instead.

### 3.9 Event Emission

```rust
#[derive(Clone, Serialize)]
struct MyEvent {
    some_id: String,
    data: String,
}

let _ = app_handle.emit("module:event-name", MyEvent { ... });
```

**Naming**: `{module}:{event-name}` with colon separator, kebab-case event names.

Examples: `terminal:output`, `terminal:exit`, `ssh:output`, `ssh:disconnected`.

---

## 4. Cross-Cutting Standards

### 4.1 Git Conventions

#### Branch Naming

```
feature/{spec-section}-{brief-description}
fix/{issue-number}-{brief-description}
refactor/{area}-{description}
```

Examples: `feature/§5-session-crud`, `fix/42-vault-lock-crash`, `refactor/ssh-error-handling`.

#### Commit Messages

```
{type}({scope}): {description}

{optional body}
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `security`.
Scopes: `vault`, `ssh`, `terminal`, `config`, `audit`, `ui`, `theme`, `i18n`, `build`.

Examples:
```
feat(vault): implement credential CRUD with AES-256-GCM encryption
fix(terminal): handle PTY EOF race condition on resize
docs(arch): add IPC contract documentation
security(ssh): implement known_hosts verification
```

### 4.2 Logging

#### Rust (Backend)

- Use the `log` crate with `tauri-plugin-log`.
- Log levels: `error` (unrecoverable), `warn` (degraded), `info` (lifecycle events), `debug` (diagnostic), `trace` (fine-grained).
- **Never log**: passwords, private keys, encryption keys, tokens.
- **Do log**: Connection events (host, port — no credentials), vault lock/unlock (no password), errors with context.

#### TypeScript (Frontend)

- Use `console.error()` for errors that reach the component boundary.
- Use `console.warn()` for degraded states (WebGL fallback, etc.).
- No `console.log()` in committed code.

### 4.3 Testing File Conventions

| Language    | Test Location                        | Naming                     |
|-------------|--------------------------------------|----------------------------|
| Rust        | Inline `#[cfg(test)] mod tests`      | `fn test_feature_name()`   |
| Rust (integ)| `tests/` directory                   | `test_module_scenario.rs`  |
| TypeScript  | `__tests__/` next to source          | `ComponentName.test.tsx`   |
| E2E         | `e2e/` top-level                     | `feature.spec.ts`          |

### 4.4 Dependency Management

- **Rust**: Pin major versions in `Cargo.toml`. Run `cargo audit` in CI.
- **npm**: Pin exact versions for critical dependencies (React, Tauri API). Use `^` for utilities.
- **No vendoring** — Use package manager lockfiles (`Cargo.lock`, `package-lock.json`).
- **Minimize dependencies** — Justify every new dependency. Prefer `std` over crates where feasible.

### 4.5 Code Comments

- **No obvious comments** — Don't comment what the code does. Comment *why* when non-obvious.
- **Region markers** — Use `// ── Section Name ──` for Rust module sections (established pattern).
- **TODO markers** — Use `// TODO: description` for known gaps. Include spec reference if applicable.
- **Section comments in App.tsx** — Use `// ─── Region X: Name ───` to mark layout regions.

---

## 5. Anti-Patterns to Avoid

| Anti-Pattern                        | Correct Pattern                                |
|-------------------------------------|------------------------------------------------|
| Hardcoded color values in components | Use Tailwind token classes                    |
| Hardcoded strings in JSX            | Use `t()` from i18next                        |
| `useAppStore()` (no selector)       | `useAppStore((s) => s.field)` (selector)      |
| `any` type in TypeScript            | `unknown` with type guard, or specific type   |
| `.unwrap()` in Tauri commands       | `?` operator with proper error type           |
| Props drilling through 3+ levels    | Zustand store subscription                    |
| CSS-in-JS or styled-components      | Tailwind utility classes                      |
| Scattered type definitions          | All types in `src/types/index.ts`             |
| Sequential IDs                      | UUID v4 from `uuid` crate/package             |
| Logging secrets                     | Log context without sensitive data            |
| `console.log` in production code    | `console.error`/`console.warn` for real issues|
| Relative imports from far paths     | `@/` path alias for all internal imports      |
| Monolithic components               | One responsibility per component file         |
| Class components                    | Function components with hooks                |
