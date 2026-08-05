export interface KeyValueStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/** Read a canonical value, copying a validated legacy fallback without deleting it. */
export function readMigratedStorageValue(
  storage: KeyValueStorage,
  canonicalKey: string,
  legacyKey: string,
  normalizeLegacy: (value: string) => string | null = (value) => value,
): string | null {
  try {
    const canonical = storage.getItem(canonicalKey);
    if (canonical != null) return canonical;
    const legacy = storage.getItem(legacyKey);
    if (legacy == null) return null;
    let normalized: string | null;
    try {
      normalized = normalizeLegacy(legacy);
    } catch {
      return null;
    }
    if (normalized == null) return null;
    try {
      storage.setItem(canonicalKey, normalized);
    } catch {
      // The legacy value remains the recoverable source.
    }
    return normalized;
  } catch {
    return null;
  }
}
