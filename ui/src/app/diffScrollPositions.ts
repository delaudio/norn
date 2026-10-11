export interface DiffScrollPosition {
  top: number;
  left: number;
}
/** Bounded transient positions, keyed by target/run/file/content/mode identity. */
export class DiffScrollPositions {
  private entries = new Map<string, DiffScrollPosition>();
  get(identity: string): DiffScrollPosition | undefined {
    return this.entries.get(identity);
  }
  set(identity: string, position: DiffScrollPosition) {
    if (!Number.isFinite(position.top) || !Number.isFinite(position.left)) return;
    this.entries.delete(identity);
    this.entries.set(identity, {
      top: Math.max(0, position.top),
      left: Math.max(0, position.left),
    });
    while (this.entries.size > 32) {
      const first = this.entries.keys().next().value;
      if (first === undefined) break;
      this.entries.delete(first);
    }
  }
  clear() {
    this.entries.clear();
  }
}
