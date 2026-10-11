import type { ScrollBoxRenderable } from "@opentui/core";
import { useKeyboard, usePaste } from "@opentui/react";
import { useEffect, useRef, useSyncExternalStore } from "react";
import type { BackendClient } from "../../transport/backendClient";
import type { SettingsStore } from "../settingsStore";
import type { ShellStore } from "../store";
import { theme } from "../theme";

export function SettingsPanel({
  settings,
  shell,
  client,
}: {
  settings: SettingsStore;
  shell: ShellStore;
  client: BackendClient;
}) {
  const state = useSyncExternalStore(settings.subscribe, settings.getSnapshot);
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  const fields = settings.fields();
  useEffect(() => {
    scroll.current?.scrollChildIntoView(`setting-${state.selected}`);
  }, [state.selected]);
  useKeyboard((key) => {
    key.preventDefault();
    key.stopPropagation();
    if (state.themePreview) {
      if (key.ctrl && key.name === "y") void settings.stageImportedTheme(client);
      else if (key.name === "escape") settings.dismissImportedTheme();
      return;
    }
    if (state.clearConfirmation) {
      if (key.name === "y") void settings.clearCredential(client);
      else if (key.name === "escape" || key.name === "n") settings.dismissClear();
      return;
    }
    if (state.editing) {
      if (key.name === "escape") settings.cancelEdit();
      else if (key.name === "return") settings.finishEdit();
      else if (key.name === "backspace") settings.backspace();
      else if (key.ctrl && key.name === "u") settings.clearEdit();
      else if (!key.ctrl && !key.meta && !["tab", "up", "down", "left", "right"].includes(key.name))
        settings.append(key.sequence);
      return;
    }
    if (key.ctrl && key.name === "s") {
      void settings
        .save(client)
        .then(async (value) => {
          if (value) await shell.applySettings(client, value);
        })
        .catch((error) => shell.setError(error instanceof Error ? error.message : String(error)));
    } else if (key.ctrl && key.name === "d") shell.toggleData();
    else if (key.ctrl && key.name === "t") void settings.previewImportedTheme(client);
    else if (key.ctrl && key.name === "r") void settings.refreshReadiness(client);
    else if (key.ctrl && key.name === "k") void settings.saveCredential(client);
    else if (key.name === "escape" || key.name === "q") shell.toggleSettings();
    else if (key.name === "j" || key.name === "down" || key.name === "tab") settings.move(1);
    else if (key.name === "k" || key.name === "up") settings.move(-1);
    else if (key.name === "return") settings.beginEdit();
    else if (key.name === "a") settings.addRepository();
    else if (key.name === "d") settings.removeRepository();
    else if (key.name === "x") settings.requestClear();
  });
  usePaste((event) => {
    event.preventDefault();
    if (state.editing && !state.clearConfirmation)
      settings.append(new TextDecoder().decode(event.bytes));
  });
  return (
    <box flexDirection="column" flexGrow={1} paddingX={1} backgroundColor={theme.background}>
      <text fg={theme.primary} height={1} flexShrink={0} truncate>
        Settings · Enter edit/cycle · Esc discard and close
      </text>
      <text fg={theme.muted} height={1} flexShrink={0} truncate>
        Ctrl+D user data · Ctrl+S preferences · Ctrl+K credential · a add repo · d remove repo · x
        clear keychain
      </text>
      <text fg={theme.muted} height={1} flexShrink={0} truncate>
        Secrets stay in the OS store. Existing environment credentials are preserved.
      </text>
      {state.loading ? <text fg={theme.muted}>Loading preferences…</text> : null}
      {state.credentialsLoading ? (
        <text fg={theme.muted}>Checking OS credential status…</text>
      ) : null}
      {state.saving ? (
        <text fg={theme.warning}>Saving… wait for confirmation before retrying.</text>
      ) : null}
      {state.error ? (
        <text fg={theme.error} height={3} flexShrink={0}>
          {state.error}
        </text>
      ) : null}
      {state.notice ? (
        <text fg={theme.warning} height={2} flexShrink={0}>
          {state.notice}
        </text>
      ) : null}
      <text fg={theme.muted} height={1} flexShrink={0} truncate>
        Ctrl+T preview imported theme · Ctrl+Y stage · Ctrl+S save
      </text>
      {state.themeLoading ? (
        <text fg={theme.warning} height={1} flexShrink={0}>
          Reading imported theme…
        </text>
      ) : null}
      {state.themePreview ? (
        <text
          fg={theme.warning}
          height={2}
          flexShrink={0}
        >{`Theme draft: ${state.themePreview.draftTheme} → imported ${state.themePreview.value}. Ctrl+Y stage · Esc dismiss`}</text>
      ) : null}
      <text fg={theme.secondary} height={1} flexShrink={0} truncate>
        {state.credentials
          .map(
            (status) =>
              `${status.provider}: ${status.available ? status.source : "not configured"}`,
          )
          .join(" · ")}
      </text>
      <text fg={theme.muted} height={1} flexShrink={0} truncate>
        {state.readinessLoading
          ? "Checking tool paths…"
          : state.readiness
            ? `Tools: ${state.readiness.tools.map((tool) => `${tool.name} ${tool.available ? "found" : "missing"}`).join(" · ")} · Ctrl+R refresh`
            : "Ctrl+R checks tool paths. Presence does not verify authentication."}
      </text>
      {state.readiness ? (
        <text fg={theme.warning} height={1} flexShrink={0} truncate>
          {state.readiness.credentialStore === "unsupported"
            ? "Native credential store unavailable on this build; existing environment credentials work. No plaintext fallback."
            : `Selected agent: ${state.readiness.selectedAgent} · tool presence does not verify login or provider connection.`}
        </text>
      ) : null}
      {state.clearConfirmation ? (
        <text
          fg={theme.warning}
        >{`Remove ${state.credentialProvider} keychain credential? y confirm · n/Esc dismiss`}</text>
      ) : null}
      <scrollbox ref={scroll} flexGrow={1} minHeight={0}>
        {fields.map((field, index) => (
          <box
            id={`setting-${index}`}
            key={field.id}
            flexDirection="column"
            flexShrink={0}
            backgroundColor={index === state.selected ? theme.element : theme.background}
          >
            <text
              fg={index === state.selected ? theme.primary : theme.text}
            >{`${index === state.selected ? "›" : " "} ${field.label}: ${state.editing && index === state.selected ? settings.editDisplay() : field.display || "(default / empty)"}`}</text>
          </box>
        ))}
      </scrollbox>
      <text fg={theme.muted} height={1} flexShrink={0} truncate>
        {state.editing
          ? "Typing changes this draft · Ctrl+U clear · Enter keep · Esc undo field"
          : "Cancel discards unsaved fields. Saving preferences never writes credentials."}
      </text>
    </box>
  );
}
