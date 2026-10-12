import type {
  RepositoryEditor,
  RepositoryEditorPrepareParams,
} from "../../../src/protocol/generated/protocol";
import { validExploreParams } from "./explorationValidation";
import { sameLocalValue } from "./localValidation";
import { validPublicationDTO } from "./publicationValidation";

export function validEditorParams(type: string, params: object): boolean {
  return validExploreParams(type, params);
}
export function validEditorResult(
  type: string,
  result: Record<string, unknown>,
  params: object,
  expected?: RepositoryEditorPrepareParams | RepositoryEditor,
): boolean {
  if (
    Buffer.byteLength(JSON.stringify(result)) > 16384 ||
    !validPublicationDTO(type, result) ||
    !sameLocalValue(result.request, params)
  )
    return false;
  if (type === "RepositoryEditorPrepared") {
    return result.token === null
      ? result.integration === null &&
          result.expiresInMs === 0 &&
          typeof result.message === "string"
      : typeof result.integration === "string" && (result.expiresInMs as number) > 0;
  }
  if (!expected) return false;
  const editor = result.editor as RepositoryEditor;
  if (
    !sameLocalValue(editor.context, expected.context) ||
    editor.path !== expected.path ||
    editor.line !== expected.line ||
    editor.mode !== expected.mode
  )
    return false;
  if (
    "launchId" in expected &&
    (editor.launchId !== expected.launchId || editor.integration !== expected.integration)
  )
    return false;
  if (
    (editor.mode === "graphical" && editor.state === "running") ||
    (editor.mode === "terminal" && editor.state === "launched")
  )
    return false;
  if (["starting", "running", "launched"].includes(editor.state) && editor.exitCode !== null)
    return false;
  if (
    (editor.state === "exited" && editor.exitCode !== 0) ||
    (editor.state === "failed" && editor.exitCode === 0)
  )
    return false;
  if (
    "state" in expected &&
    ["exited", "failed", "canceled"].includes(expected.state) &&
    editor.state !== expected.state
  )
    return false;
  return true;
}
