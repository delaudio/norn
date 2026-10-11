import { TextBuffer, TextBufferView, type WidthMethod } from "@opentui/core";

export interface RowGeometry {
  offsets: number[];
  width: number;
}

// Use the renderer's native word wrapper instead of estimating text.length:
// wide characters, combining marks, tabs and word boundaries change row height.
export function measureDiffRows(
  texts: string[][],
  width: number,
  method: WidthMethod,
): RowGeometry {
  const heights = texts.map(() => 1);
  let maximumWidth = width;
  // Split views also contain full-width metadata rows. Their first row must
  // not accidentally select unified geometry for all following pairs.
  const columns = texts.reduce((maximum, row) => Math.max(maximum, row.length), 1);
  for (let column = 0; column < columns; column += 1) {
    const buffer = TextBuffer.create(method);
    const view = TextBufferView.create(buffer);
    try {
      buffer.setText(
        texts
          .map((row) => (columns === 2 && row.length === 1 ? "" : (row[column] ?? "")))
          .join("\n"),
      );
      view.setWrapMode(columns === 2 ? "word" : "none");
      view.setWrapWidth(
        columns === 2
          ? Math.max(1, column === 0 ? Math.floor(width / 2) : Math.ceil(width / 2))
          : width,
      );
      if (columns === 1) maximumWidth = Math.max(width, view.lineInfo.lineWidthColsMax);
      const counts = new Array<number>(texts.length).fill(0);
      for (const source of view.getLineSources(0, view.getVirtualLineCount())) {
        if (source < counts.length) counts[source] = (counts[source] ?? 0) + 1;
      }
      for (let index = 0; index < heights.length; index += 1)
        heights[index] = Math.max(heights[index] ?? 1, counts[index] ?? 1);
    } finally {
      view.destroy();
      buffer.destroy();
    }
  }
  const offsets = [0];
  for (const height of heights) offsets.push((offsets.at(-1) ?? 0) + height);
  return { offsets, width: maximumWidth };
}

export function visibleDiffRows(offsets: number[], top: number, height: number, overscan = 8) {
  const count = Math.max(0, offsets.length - 1);
  const lowerBound = (position: number) => {
    let left = 0;
    let right = count;
    while (left < right) {
      const middle = (left + right) >>> 1;
      if ((offsets[middle + 1] ?? 0) <= position) left = middle + 1;
      else right = middle;
    }
    return left;
  };
  const start = Math.max(0, lowerBound(Math.max(0, top)) - overscan);
  const end = Math.min(count, lowerBound(Math.max(0, top) + Math.max(1, height)) + overscan + 1);
  return { start, end };
}
