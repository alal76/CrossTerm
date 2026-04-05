# CrossTerm — Design Specification

| Field          | Value                                |
|----------------|--------------------------------------|
| Spec ID        | DESIGN-CROSSTERM-001                 |
| Version        | 1.0                                  |
| Status         | Active                               |
| Last Updated   | 2026-04-05                           |
| Parent Spec    | SPEC-CROSSTERM-001, ARCH-CROSSTERM-001 |

---

## 1. Design System Overview

CrossTerm implements a **token-driven design system** where all visual properties are defined as named tokens, consumed through CSS custom properties, and mapped to Tailwind utility classes. No hardcoded colors, spacing, or typography values exist in component code.

---

## 2. Token Architecture

### 2.1 Three-Layer Indirection

```
Layer 1: Theme JSON files (themes/dark.json, light.json)
    ↓  define concrete values
Layer 2: CSS Custom Properties (index.css :root / .light)
    ↓  consumed by
Layer 3: Tailwind Config (tailwind.config.ts)
    ↓  generates utility classes
Components: <div className="bg-surface-primary text-text-secondary" />
```

### 2.2 Token Categories

All tokens are defined in `src/types/index.ts` as the `ThemeTokens` interface.

| Category       | Tokens                                                                                         | Purpose |
|----------------|-----------------------------------------------------------------------------------------------|---------|
| **Surface**    | `surface-primary`, `surface-secondary`, `surface-elevated`, `surface-sunken`, `surface-overlay` | Background colors for layout regions |
| **Text**       | `text-primary`, `text-secondary`, `text-disabled`, `text-inverse`, `text-link`                  | Text hierarchy |
| **Border**     | `border-default`, `border-subtle`, `border-strong`, `border-focus`                              | Dividers, outlines, focus rings |
| **Interactive**| `interactive-default`, `interactive-hover`, `interactive-active`, `interactive-disabled`         | Button and control states |
| **Status**     | `status-connected`, `status-disconnected`, `status-connecting`, `status-idle`                   | Connection state indicators |
| **Accent**     | `accent-primary`, `accent-secondary`                                                           | Active indicators, badges, focus |
| **Terminal**   | `terminal-foreground`, `terminal-background`, `terminal-cursor`, `terminal-selection`, `terminal-ansi-0` through `terminal-ansi-15` | Terminal emulator colors |

### 2.3 Tailwind Token Mapping

The `tailwind.config.ts` maps every token to its CSS variable:

```typescript
colors: {
  surface: {
    primary: "var(--surface-primary)",    // → bg-surface-primary
    secondary: "var(--surface-secondary)", // → bg-surface-secondary
    // ...
  },
  text: {
    primary: "var(--text-primary)",        // → text-text-primary
    // ...
  },
  // ...
}
```

**Rule**: Components MUST use Tailwind token classes. Direct CSS variable references (`var(--xxx)`) are only used in `index.css` and inline styles for dynamic values.

---

## 3. Layout System

### 3.1 Region-Based Layout (Six Regions)

The application shell is composed of six named regions, implemented as distinct React components in `App.tsx`:

```
┌─────────────────────────────────────────────────┐
│ A: TitleBar (h-10, shrink-0)                    │
├────────┬────────────────────────────────────────┤
│        │ B: TabBar (h-[38px], shrink-0)         │
│ C:     ├────────────────────────────────────────┤
│Sidebar │ D: SessionCanvas (flex-1)              │
│(w-60 / ├────────────────────────────────────────┤
│ w-12)  │ E: BottomPanel (h-[30%], conditional)  │
├────────┴────────────────────────────────────────┤
│ F: StatusBar (h-7, shrink-0)                    │
└─────────────────────────────────────────────────┘
```

| Region | Component       | Layout Strategy      | Sizing                            |
|--------|-----------------|---------------------|-----------------------------------|
| A      | `TitleBar`      | Flex row            | Fixed `h-10`, `shrink-0`          |
| B      | `TabBar`        | Flex row, scrollable| Fixed `h-[38px]`, `shrink-0`      |
| C      | `Sidebar`       | Flex column         | `w-60` expanded, `w-12` collapsed |
| D      | `SessionCanvas` | Flex fill           | `flex-1`, fills remaining space   |
| E      | `BottomPanel`   | Flex column         | `h-[30%]`, min `120px`, toggleable|
| F      | `StatusBar`     | Flex row            | Fixed `h-7`, `shrink-0`           |

### 3.2 Sidebar Design

The sidebar uses a **dual-column layout**:

1. **Icon Rail** (always visible, `w-12`): Vertical column of mode buttons.
2. **Content Panel** (visible when expanded): Shows the active mode's content.

**Sidebar Modes** (enum `SidebarMode`):
- `Sessions` — Session tree with folders, favorites, search.
- `Snippets` — Command snippet library.
- `Tunnels` — Port forward manager.

**Interaction pattern**: Clicking an active mode icon collapses the sidebar. Clicking an inactive mode switches content. This mirrors VS Code's activity bar behavior.

**Active mode indicator**: A 2px accent-colored bar on the left edge of the active icon button, plus `bg-surface-elevated` background.

### 3.3 Responsive Behavior

The sidebar auto-collapses to icon rail when `window.innerWidth < 900px`, controlled by `appStore.setWindowDimensions()` in the `resize` event listener.

| Breakpoint      | Width         | Sidebar State     |
|-----------------|---------------|-------------------|
| Compact         | < 600px       | Hidden            |
| Medium          | 600–899px     | Icon rail only    |
| Expanded        | 900–1440px    | Full sidebar      |
| Large           | > 1440px      | Full + dual mode  |

---

## 4. Component Design Patterns

### 4.1 Region Components (Layout)

Region components (TitleBar, TabBar, Sidebar, SessionCanvas, BottomPanel, StatusBar) are defined directly in `App.tsx`. They are **not exported** — they are internal to the app shell.

**Pattern**: Each region component:
- Subscribes to its relevant Zustand store slices via selectors.
- Uses the `useTranslation()` hook for all user-facing strings.
- Returns a `<div>` with fixed height/width (for chrome) or `flex-1` (for canvas).

### 4.2 Feature Components (Domain)

Feature components live in subdirectories under `src/components/`:

| Directory       | Components              | Responsibility                           |
|-----------------|-------------------------|------------------------------------------|
| `Terminal/`     | TerminalView, TerminalTab | xterm.js lifecycle, PTY bridge           |
| `SessionTree/`  | SessionTree, SessionEditor | Session list, CRUD forms                |
| `Settings/`     | SettingsPanel           | Settings categories and controls         |
| `SftpBrowser/`  | SftpBrowser             | Dual-pane file browser                   |
| `Shared/`       | CommandPalette, QuickConnect, Toast | Cross-cutting UI overlays      |
| `Vault/`        | CredentialManager, VaultUnlock | Credential vault UI              |

### 4.3 Empty State Pattern

Every panel implements a consistent empty state using the `EmptyPanel` component:

```tsx
function EmptyPanel({ icon, message }: { icon: React.ReactNode; message: string }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-8 text-center">
      {icon}
      <p className="text-xs text-text-secondary px-2">{message}</p>
    </div>
  );
}
```

**Design rules for empty states:**
- Always show a relevant icon (32px, `text-text-disabled` color).
- Include an actionable message from the i18n locale.
- Optionally include a primary action button (e.g., "New Session").

### 4.4 Status Dot Pattern

Connection status is indicated by a colored dot using the `StatusDot` component:

```tsx
function StatusDot({ status }: { status: ConnectionStatus }) {
  const colorClass = {
    [ConnectionStatus.Connected]: "bg-status-connected",     // green
    [ConnectionStatus.Disconnected]: "bg-status-disconnected", // red
    [ConnectionStatus.Connecting]: "bg-status-connecting",     // amber
    [ConnectionStatus.Idle]: "bg-status-idle",                 // grey
  }[status];
  return <span className={clsx("inline-block w-2 h-2 rounded-full", colorClass)} />;
}
```

Used in: Tab bar (per tab), Status bar, Session tree entries.

---

## 5. Tab System Design

### 5.1 Tab Data Model

```typescript
interface Tab {
  id: string;           // UUID
  sessionId: string;    // References Session.id
  title: string;        // Display name
  sessionType: SessionType;
  status: ConnectionStatus;
  pinned: boolean;      // Pinned tabs are icon-only, locked left
  order: number;        // Sort order
}
```

### 5.2 Tab Bar Behavior

- **Pinned tabs**: Rendered first, icon-only (32px wide), sorted by order.
- **Unpinned tabs**: Rendered after pinned, 120–240px width, truncated with ellipsis.
- **Active tab**: Top border (2px `accent-primary`), primary text color, elevated background.
- **Hover**: Reveals close `×` button (opacity transition).
- **Middle-click**: Closes tab.
- **Content**: Session-type emoji icon + title + status dot + close button.

### 5.3 Session Type Icons

Session types use emoji icons mapped via `SESSION_TYPE_ICONS`:

| Type            | Icon | Type            | Icon |
|-----------------|------|-----------------|------|
| `ssh`           | ⌨    | `vnc`           | 🖥   |
| `sftp`          | 📁   | `local_shell`   | ⌨    |
| `rdp`           | 🖥   | `serial`        | 🔌   |
| `telnet`        | 📡   | `cloud_shell`   | ☁    |
| `wsl`           | 🐧   | `kubernetes_exec`| ☸   |
| `docker_exec`   | 🐳   | `web_console`   | 🌐   |
| `scp`           | 📤   |                 |      |

---

## 6. Terminal Emulator Design

### 6.1 xterm.js Integration

The `TerminalView` component wraps xterm.js with:

| Addon          | Purpose                          | Fallback               |
|----------------|----------------------------------|------------------------|
| `FitAddon`     | Auto-size terminal to container  | —                      |
| `WebglAddon`   | GPU-accelerated rendering        | Canvas renderer        |
| `SearchAddon`  | Incremental search in scrollback | —                      |
| `WebLinksAddon`| Clickable URL detection          | —                      |

### 6.2 Terminal Theme Binding

Terminal colors are read from CSS custom properties at initialization:

```typescript
function getTerminalTheme(): Record<string, string> {
  return {
    foreground: getCSSVar("--terminal-fg"),
    background: getCSSVar("--terminal-bg"),
    cursor: getCSSVar("--terminal-cursor"),
    // ... ANSI 0-15
  };
}
```

This ensures terminal colors update when the theme changes (on re-mount).

### 6.3 Terminal Lifecycle

```
Mount → new Terminal() → loadAddons → open(container) → fitAddon.fit()
  → onData(input) → invoke("terminal_write")
  → listen("terminal:output") → term.write(data)
  → ResizeObserver → fitAddon.fit() → invoke("terminal_resize")
Unmount → dispose observers, unlisten events, term.dispose()
```

### 6.4 Default Terminal Configuration

| Setting        | Value                        | Source          |
|----------------|------------------------------|-----------------|
| Font family    | `JetBrains Mono, monospace`  | Hard-coded      |
| Font size      | 14px                         | Hard-coded      |
| Line height    | 1.2                          | Hard-coded      |
| Scrollback     | 10,000 lines                 | Hard-coded      |
| Cursor style   | Block, blinking              | Hard-coded      |
| Cursor blink   | true                         | Hard-coded      |

**Note**: These should migrate to `Settings` in the config store for user configurability (per SPEC §6.2).

---

## 7. Theme Design

### 7.1 Shipped Themes

| Theme          | Variant | Palette Summary                              |
|----------------|---------|----------------------------------------------|
| CrossTerm Dark | Dark    | Tokyo Night-inspired. Neutral blue-grey surfaces, teal accent (`#2dd4bf`), blue secondary (`#7aa2f7`). |
| CrossTerm Light| Light   | Warm ivory surfaces, indigo accent (`#5b6ee1`), teal secondary. |

### 7.2 Dark Theme Token Values (Excerpt)

| Token               | Value         | Visual                               |
|----------------------|---------------|--------------------------------------|
| `surface-primary`   | `#1a1b26`     | Base background (Tokyo Night)        |
| `surface-secondary` | `#222337`     | Sidebar, tab bar background          |
| `surface-elevated`  | `#2a2b3d`     | Hover states, active icons           |
| `surface-sunken`    | `#13141f`     | Terminal canvas background           |
| `text-primary`      | `#c8d3f5`     | Primary text                         |
| `text-secondary`    | `#828bb8`     | Secondary/muted text                 |
| `accent-primary`    | `#2dd4bf`     | Active tab border, sidebar indicator |
| `status-connected`  | `#22c55e`     | Green dot                            |
| `status-disconnected`| `#ef4444`    | Red dot                              |

### 7.3 Theme Switching Mechanism

```typescript
// In App.tsx useEffect:
const root = document.documentElement;
if (theme === ThemeVariant.Light) {
  root.classList.add("light");
} else {
  root.classList.remove("light");
}
```

CSS variables for the light theme are defined under `.light { ... }` in `index.css`. Tailwind's `darkMode: "class"` enables class-based dark mode.

---

## 8. Animation & Motion

### 8.1 CSS Custom Properties for Timing

```css
:root {
  --duration-micro: 100ms;
  --duration-short: 150ms;
  --duration-medium: 250ms;
  --duration-long: 400ms;
  --ease-default: cubic-bezier(0.4, 0, 0.2, 1);
  --ease-decelerate: cubic-bezier(0, 0, 0.2, 1);
  --ease-accelerate: cubic-bezier(0.4, 0, 1, 1);
}
```

### 8.2 Animation Classes

| Class             | Effect                                    | Duration |
|-------------------|-------------------------------------------|----------|
| `animate-fade-in` | Opacity 0→1                              | 200ms    |
| `animate-slide-bottom` | Translate Y 100%→0 + opacity       | 250ms    |

### 8.3 Transition Usage

Components apply transitions via Tailwind's `transition-colors` and inline `transitionDuration` using CSS variable references:

```tsx
style={{
  transitionDuration: "var(--duration-medium)",
  transitionTimingFunction: "var(--ease-default)",
}}
```

---

## 9. Internationalization Design

### 9.1 Architecture

- **Library**: i18next with `react-i18next` integration.
- **Locale files**: JSON format in `src/i18n/`. Currently: `en.json`.
- **Initialization**: Eager load. No lazy loading needed for single-locale MVP.
- **Fallback**: Always `en`.

### 9.2 String Usage Pattern

```tsx
const { t } = useTranslation();
// In JSX:
<span>{t("sidebar.sessions")}</span>
<p>{t("emptyCanvas.hint", { shortcut: "Ctrl+T" })}</p>
```

### 9.3 Locale Key Structure

Keys are namespaced by UI region and feature:

```json
{
  "sidebar": { "sessions": "...", "snippets": "...", "tunnels": "..." },
  "tabs": { "newTab": "..." },
  "emptyCanvas": { "title": "...", "hint": "..." },
  "sessions": { "emptyState": "...", "newSession": "...", "noResults": "..." },
  "statusBar": { "profile": "...", "encoding": "...", "noTunnels": "..." },
  "status": { "connected": "...", "disconnected": "...", "connecting": "...", "idle": "..." },
  "vault": { "unlock": "...", "create": "...", "masterPassword": "..." },
  "settings": { "title": "..." },
  "themes": { "title": "..." }
}
```

**Rule**: No hardcoded user-facing strings in components. All strings come from `t()`.

---

## 10. Iconography

### 10.1 Icon Library

**Primary**: Lucide React (MIT licensed). Consistent 24px default with 2px stroke weight.

### 10.2 Icon Sizes by Context

| Context              | Size  | Lucide `size` prop |
|----------------------|-------|--------------------|
| Sidebar icon rail    | 20px  | `20`               |
| Tab bar              | 16px  | `16` (via emoji)   |
| Sidebar content      | 14px  | `14`               |
| Status bar           | 11px  | `11`               |
| Empty state hero     | 32px  | `32`               |
| Action buttons       | 16px  | `16`               |
| Plus/close buttons   | 12-16px | `12`–`16`        |

### 10.3 Icon Color Convention

| Context          | Color Class          |
|------------------|----------------------|
| Default/inactive | `text-text-secondary` |
| Hover            | `text-text-primary`   |
| Active/selected  | `text-accent-primary` |
| Disabled         | `text-text-disabled`  |

---

## 11. Interaction Patterns

### 11.1 Command Palette (⌘⇧P)

- **Trigger**: Global keyboard shortcut `Cmd+Shift+P` (Mac) / `Ctrl+Shift+P` (Win/Linux).
- **UI**: Full-width overlay at top of window, `surface-overlay` backdrop.
- **Search**: Fuzzy-match filter over a list of `PaletteAction` items.
- **Navigation**: Arrow keys to select, Enter to execute, Escape to close.
- **Actions**: Each action has id, label, icon, optional shortcut, and handler.

### 11.2 Quick Connect (⌘⇧N)

- **Trigger**: `Cmd+Shift+N` / `Ctrl+Shift+N`.
- **UI**: Compact modal with host/port/username/auth fields.
- **Behavior**: Parse `ssh user@host` format for quick SSH connections.

### 11.3 Toast Notifications

- **Position**: Bottom-right corner (desktop).
- **Variants**: `success` (green), `info` (accent), `warning` (amber), `error` (red).
- **Auto-dismiss**: Success/info auto-dismiss (4–5s). Warning/error persist.
- **Stack**: Max 3 visible, older queue behind.

### 11.4 Bottom Panel Toggle

- **Trigger**: Keyboard shortcut (Ctrl+J) or close button.
- **Modes**: SFTP, Snippets, Audit Log, Search — tab-switcher at top.
- **Size**: 30% of canvas height, min 120px.
- **Drag handle**: Visual indicator for resize (not yet functional).
