// Serialized native OpenTUI frame fixtures, produced by the real renderer.

export interface TerminalFrameSpan {
  text: string;
  width: number;
  fg: string;
  bg: string;
  /** Raw OpenTUI attribute bits, retained so any attribute regression is caught. */
  attributes: number;
  bold: boolean;
  italic: boolean;
  underline: boolean;
}

export interface TerminalFrameLine {
  spans: TerminalFrameSpan[];
}

export interface TerminalFrame {
  cols: number;
  rows: number;
  lines: TerminalFrameLine[];
}

export type TerminalFrameSet = Record<string, TerminalFrame>;
