export function formatVersion(version: string | null | undefined): string {
  if (!version) return "Unknown";
  return version;
}

export function isVersionNewer(current: string, available: string): boolean {
  const currentParts = current.split(".").map(Number);
  const availableParts = available.split(".").map(Number);
  const maxLen = Math.max(currentParts.length, availableParts.length);

  for (let i = 0; i < maxLen; i++) {
    const a = currentParts[i] || 0;
    const b = availableParts[i] || 0;
    if (b > a) return true;
    if (b < a) return false;
  }
  return false;
}
