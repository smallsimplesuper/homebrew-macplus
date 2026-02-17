const UNITS = ["B", "KB", "MB", "GB"] as const;

function scaleIndex(bytes: number): number {
  if (bytes <= 0) return 0;
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return Math.min(i, UNITS.length - 1);
}

function formatAtScale(bytes: number, idx: number): string {
  const val = bytes / 1024 ** idx;
  return idx === 0 ? val.toFixed(0) : val.toFixed(1);
}

export function formatBytes(bytes: number): string {
  const idx = scaleIndex(bytes);
  return `${formatAtScale(bytes, idx)} ${UNITS[idx]}`;
}

export function formatDownloadProgress(
  downloaded: number,
  total: number | null | undefined,
): string {
  if (total == null || total === 0) return formatBytes(downloaded);
  // Use the unit scale of total for both values
  const idx = scaleIndex(total);
  return `${formatAtScale(downloaded, idx)} / ${formatAtScale(total, idx)} ${UNITS[idx]}`;
}
