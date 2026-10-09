/** @jsxImportSource react */
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { TerminalFrameSet } from "../frame";
import framesJson from "../generated/frames.json";
import { TerminalPreview } from "../TerminalPreview";

const frames = framesJson as TerminalFrameSet;
const frameIds = Object.keys(frames);
const first = frameIds[0] ?? "";

const meta = {
  title: "Norn/Terminal frames",
  component: TerminalPreview,
  parameters: { layout: "centered", controls: { disable: true } },
  args: { frameId: first },
} satisfies Meta<typeof TerminalPreview>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ready80x24: Story = { args: { frameId: "shell-ready-80x24" } };
export const Ready50x15: Story = { args: { frameId: "shell-ready-50x15" } };
export const Ready120x30: Story = { args: { frameId: "shell-ready-120x30" } };
export const Empty: Story = { args: { frameId: "shell-empty-80x24" } };
export const Connecting: Story = { args: { frameId: "shell-connecting-80x24" } };
export const ErrorState: Story = { args: { frameId: "shell-error-80x24" } };
export const Closed: Story = { args: { frameId: "shell-closed-80x24" } };
export const Review: Story = { args: { frameId: "shell-review-80x24" } };
export const ReviewTargets: Story = { args: { frameId: "shell-review-targets-120x30" } };
export const StaleAnchor: Story = { args: { frameId: "shell-stale-anchor-80x24" } };

export const Gallery: Story = {
  args: { frameId: first },
  render: () => (
    <div style={{ display: "flex", flexWrap: "wrap", gap: 24 }}>
      {frameIds.map((id) => (
        <div key={id}>
          <div style={{ color: "#808080", font: "12px monospace", marginBottom: 4 }}>{id}</div>
          <TerminalPreview frameId={id} />
        </div>
      ))}
    </div>
  ),
};
