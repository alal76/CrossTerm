export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        surface: {
          primary: "var(--surface-primary)",
          secondary: "var(--surface-secondary)",
          elevated: "var(--surface-elevated)",
          sunken: "var(--surface-sunken)",
          overlay: "var(--surface-overlay)",
        },
        text: {
          primary: "var(--text-primary)",
          secondary: "var(--text-secondary)",
          disabled: "var(--text-disabled)",
          inverse: "var(--text-inverse)",
          link: "var(--text-link)",
        },
        border: {
          default: "var(--border-default)",
          subtle: "var(--border-subtle)",
          strong: "var(--border-strong)",
          focus: "var(--border-focus)",
        },
        interactive: {
          default: "var(--interactive-default)",
          hover: "var(--interactive-hover)",
          active: "var(--interactive-active)",
          disabled: "var(--interactive-disabled)",
        },
        status: {
          connected: "var(--status-connected)",
          disconnected: "var(--status-disconnected)",
          connecting: "var(--status-connecting)",
          idle: "var(--status-idle)",
        },
        accent: {
          primary: "var(--accent-primary)",
          secondary: "var(--accent-secondary)",
        },
      },
      fontFamily: {
        ui: ["Inter", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "monospace"],
      },
      spacing: {
        "space-1": "4px",
        "space-2": "8px",
        "space-3": "12px",
        "space-4": "16px",
        "space-6": "24px",
        "space-8": "32px",
        "space-12": "48px",
        "space-16": "64px",
      },
    },
  },
  plugins: [],
};
