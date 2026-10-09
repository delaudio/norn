/** @jsxImportSource react */

import type { TerminalFrame, TerminalFrameSet } from "./frame";
import framesJson from "./generated/frames.json";
import "./preview.css";

const frames = framesJson as TerminalFrameSet;

interface KeyedCell {
  key: string;
  span: TerminalFrame["lines"][number]["spans"][number];
}
interface KeyedRow {
  key: string;
  cells: KeyedCell[];
}

// Precompute stable position-based keys once per frame so JSX never keys on an
// array index. Captured frames are immutable.
const keyedFrames = new Map<string, KeyedRow[]>(
  Object.entries(frames).map(([id, frame]) => [
    id,
    frame.lines.map((line, row) => ({
      key: `row-${row}`,
      cells: line.spans.map((span, index) => ({ key: `cell-${row}-${index}`, span })),
    })),
  ]),
);

export interface TerminalPreviewProps {
  frameId: string;
}

// Browser frame viewer only: it renders the captured cells, it is not a live
// OpenTUI runtime. Layout/color fidelity is asserted by the golden-frame lane,
// not by this DOM interpretation.
export function TerminalPreview({ frameId }: TerminalPreviewProps) {
  const frame: TerminalFrame | undefined = frames[frameId];
  const rows = keyedFrames.get(frameId);
  if (!frame || !rows) {
    throw new Error(`Missing native frame ${frameId}. Run \`bun run frames:generate\`.`);
  }
  return (
    <div
      data-visual-snapshot-root
      data-frame-id={frameId}
      role="img"
      aria-label={`Native terminal preview: ${frameId}, ${frame.cols} by ${frame.rows}`}
      className="terminal-frame"
      style={{ width: frame.cols * 9, height: frame.rows * 20 }}
    >
      {rows.map((row) => (
        <div className="terminal-line" key={row.key}>
          {row.cells.map((cell) => (
            <span
              key={cell.key}
              style={{
                width: cell.span.width * 9,
                color: cell.span.fg,
                background: cell.span.bg,
                fontWeight: cell.span.bold ? 700 : 400,
                fontStyle: cell.span.italic ? "italic" : "normal",
                textDecoration: cell.span.underline ? "underline" : "none",
              }}
            >
              {cell.span.text}
            </span>
          ))}
        </div>
      ))}
    </div>
  );
}
