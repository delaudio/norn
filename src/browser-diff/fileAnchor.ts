export function browserFileAnchor(path: string): string {
  return `browser-file-${encodeURIComponent(path)}`;
}
