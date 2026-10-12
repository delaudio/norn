import type {
  RepositoryOperation,
  RepositoryOperationCheckout,
  RepositoryOperationPrepared,
} from "../../../src/protocol/generated/protocol";
import { sameLocalValue } from "./localValidation";
import { validPublicationDTO } from "./publicationValidation";

const final = new Set(["completed", "failed", "canceled", "uncertain"]);
const rank: Record<RepositoryOperation["state"], number> = {
  starting: 0,
  checking: 1,
  running: 2,
  inspecting: 3,
  completed: 4,
  failed: 4,
  canceled: 4,
  uncertain: 4,
};
function reference(value: string): boolean {
  return (
    Buffer.byteLength(value) <= 1024 &&
    !value.includes("..") &&
    !value.includes("@{") &&
    !value.endsWith("/") &&
    !value.endsWith(".") &&
    !Array.from(value).some(
      (c) =>
        /\s/.test(c) ||
        c.codePointAt(0)! < 32 ||
        (c.codePointAt(0)! >= 127 && c.codePointAt(0)! <= 159),
    ) &&
    value.split("/").every((c) => c.length > 0 && !c.startsWith(".") && !c.endsWith(".lock"))
  );
}
export function validOperationParams(type: string, input: object): boolean {
  if (!validPublicationDTO(type, input)) return false;
  const params = input as Record<string, unknown>;
  if (type === "RepositoryOperationListParams")
    return !((params.offset as number) > 0 && params.version === null);
  const action = params.action as { kind: string; reference?: string } | undefined;
  return action?.kind !== "checkout" || reference(action.reference!);
}
function checkout(value: RepositoryOperationCheckout, expected?: unknown): boolean {
  if (expected && !sameLocalValue(value.context, expected)) return false;
  if (value.context && !sameLocalValue(value.context.repository, value.repository)) return false;
  if (value.state === "missing" || value.state === "invalid")
    return (
      value.context === null &&
      value.version === null &&
      value.headSha === null &&
      value.branch === null &&
      value.upstream === null &&
      value.stashSha === null
    );
  return (
    value.context !== null &&
    value.version !== null &&
    (value.state === "unborn" ? value.headSha === null : value.headSha !== null) &&
    (value.branch === null || reference(`refs/heads/${value.branch}`)) &&
    (value.upstream === null || reference(value.upstream))
  );
}
export function validOperationResult(
  type: string,
  result: Record<string, unknown>,
  params: object,
  expected?: RepositoryOperationPrepared | RepositoryOperation,
): boolean {
  try {
    if (
      Buffer.byteLength(JSON.stringify(result)) > 256 * 1024 ||
      !validPublicationDTO(type, result) ||
      !sameLocalValue(result.request, params)
    )
      return false;
    const request = params as Record<string, unknown>;
    if (type === "RepositoryOperationPage") {
      const state = result.checkout as RepositoryOperationCheckout;
      const branches = result.branches as {
        reference: string;
        name: string;
        kind: string;
        oid: string;
        current: boolean;
      }[];
      const total = result.total as number,
        offset = request.offset as number;
      if (!checkout(state) || !sameLocalValue(state.repository, request.repository)) return false;
      if (state.context === null)
        return total === 0 && branches.length === 0 && result.nextOffset === null;
      return (
        offset <= total &&
        branches.length === Math.min(request.limit as number, total - offset) &&
        (request.version === null || state.version === request.version) &&
        result.nextOffset ===
          (offset + branches.length < total ? offset + branches.length : null) &&
        new Set(branches.map((b) => b.reference)).size === branches.length &&
        branches.every(
          (b) =>
            reference(b.reference) &&
            b.reference === (b.kind === "local" ? "refs/heads/" : "refs/remotes/") + b.name &&
            (!b.current || (b.kind === "local" && b.name === state.branch)),
        ) &&
        branches.filter((b) => b.current).length <= 1
      );
    }
    if (type === "RepositoryOperationPrepared") {
      const before = result.before as RepositoryOperationCheckout;
      return checkout(before, request.context);
    }
    if (!expected) return false;
    const operation = result.operation as RepositoryOperation;
    const context = "context" in expected ? expected.context : expected.request.context;
    const action = "action" in expected ? expected.action : expected.request.action;
    if (
      !sameLocalValue(operation.context, context) ||
      !sameLocalValue(operation.action, action) ||
      !sameLocalValue(operation.before, expected.before) ||
      !checkout(operation.before, context) ||
      (operation.after !== null && !checkout(operation.after, context))
    )
      return false;
    if (operation.after !== null) {
      if (operation.stateChanged !== (operation.after.version !== operation.before.version))
        return false;
    } else if (operation.stateChanged !== null) return false;
    if (type === "RepositoryOperationApplied")
      return (
        operation.state === "starting" &&
        operation.after === null &&
        operation.exitCode === null &&
        !operation.recoveryRequired
      );
    if (
      !("operationId" in expected) ||
      operation.operationId !== expected.operationId ||
      rank[operation.state] < rank[expected.state]
    )
      return false;
    if (final.has(expected.state) && !sameLocalValue(operation, expected)) return false;
    if (
      operation.state === "completed" &&
      (operation.after === null ||
        operation.recoveryRequired ||
        (operation.exitCode !== null && operation.exitCode !== 0))
    )
      return false;
    if (operation.state === "uncertain" && !operation.recoveryRequired) return false;
    return true;
  } catch {
    return false;
  }
}
