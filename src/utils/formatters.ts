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
