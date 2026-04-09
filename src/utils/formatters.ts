// ── Locale-aware formatting utilities ──

/**
 * Format an ISO date string using Intl.DateTimeFormat.
 * Returns a locale-aware formatted date, or "—" if the input is empty.
 */
export function formatDate(
  iso: string | null | undefined,
  locale?: string,
): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";

  return new Intl.DateTimeFormat(locale ?? navigator.language, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

/**
 * Format a byte count into a human-readable file size using Intl.NumberFormat.
 * Examples: "1.2 KB", "34 MB", "2.1 GB"
 */
export function formatFileSize(
  bytes: number,
  locale?: string,
): string {
  if (bytes === 0) return "0 B";

  const units = ["B", "KB", "MB", "GB", "TB"] as const;
  const k = 1024;
  const i = Math.min(
    Math.floor(Math.log(Math.abs(bytes)) / Math.log(k)),
    units.length - 1,
  );
  const value = bytes / k ** i;

  const formatted = new Intl.NumberFormat(locale ?? navigator.language, {
    minimumFractionDigits: i === 0 ? 0 : 1,
    maximumFractionDigits: i === 0 ? 0 : 1,
  }).format(value);

  return `${formatted} ${units[i]}`;
}

/**
 * Format a number using Intl.NumberFormat with locale-aware grouping.
 */
export function formatNumber(
  num: number,
  locale?: string,
): string {
  return new Intl.NumberFormat(locale ?? navigator.language).format(num);
}

// ── Host color coding ──

/** 10 distinct, accessible colors for host-based session tagging. */
const HOST_COLORS = [
  "#ef4444", // red
  "#f97316", // orange
  "#eab308", // yellow
  "#22c55e", // green
  "#14b8a6", // teal
  "#3b82f6", // blue
  "#6366f1", // indigo
  "#a855f7", // purple
  "#ec4899", // pink
  "#06b6d4", // cyan
] as const;

/** Deterministic hash of a string to a 32-bit unsigned integer. */
function djb2(str: string): number {
  let hash = 5381;
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) + hash + (str.codePointAt(i) ?? 0)) >>> 0;
  }
  return hash;
}

/**
 * Return a consistent color for a given hostname.
 * Same host always maps to the same color.
 */
export function hostColor(host: string): string {
  if (!host) return HOST_COLORS[0];
  return HOST_COLORS[djb2(host) % HOST_COLORS.length];
}
