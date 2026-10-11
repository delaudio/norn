import type { ShellSnapshot } from "../store";
import { theme } from "../theme";

export function ConversationPanel({ state, wide }: { state: ShellSnapshot; wide: boolean }) {
  if (!state.conversationOpen) return null;
  const thread = state.threads[state.selectedThread];
  return (
    <box
      flexDirection="column"
      height={wide ? 6 : 4}
      flexShrink={0}
      backgroundColor={theme.panel}
      paddingX={1}
    >
      <text fg={theme.primary} height={1} truncate>
        {`Thread ${state.selectedThread + 1}/${state.threads.length}: ${thread?.title ?? "No saved thread"} · t select · v close`}
      </text>
      {state.focus === "threads" ? (
        <text fg={theme.muted} height={1} truncate>
          j/k choose · Enter load selected thread
        </text>
      ) : null}
      <scrollbox flexGrow={1} minHeight={0}>
        {thread?.truncated ? (
          <text fg={theme.warning}>Older messages omitted from this preview.</text>
        ) : null}
        {thread?.messages.map((message) => (
          <text key={message.id} fg={message.role === "user" ? theme.primary : theme.text}>
            {`${message.role}: ${message.content}${message.truncated ? " … [preview truncated]" : ""}`}
          </text>
        ))}
      </scrollbox>
    </box>
  );
}
