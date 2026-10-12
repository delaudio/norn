import { sameLocalValue } from "./localValidation";
import { validPublicationDTO } from "./publicationValidation";

function path(value: unknown): boolean {
  return (
    typeof value === "string" &&
    Buffer.byteLength(value) <= 4096 &&
    value.length > 0 &&
    !value.startsWith("/") &&
    !value.includes("\0") &&
    value.split("/").every((part) => part.length > 0 && part !== "." && part !== "..")
  );
}
export function validExploreParams(type: string, input: object): boolean {
  if (!validPublicationDTO(type, input)) return false;
  const params = input as Record<string, unknown>;
  if (params.path !== undefined && !path(params.path)) return false;
  if (typeof params.query === "string" && Buffer.byteLength(params.query) > 256) return false;
  if (
    type === "RepositoryExploreFilesParams" &&
    typeof params.query === "string" &&
    Array.from(params.query).some((character) => {
      const code = character.charCodeAt(0);
      return code < 32 || (code >= 127 && code <= 159);
    })
  )
    return false;
  if (
    type === "RepositoryExploreFindParams" &&
    typeof params.query === "string" &&
    params.query.includes("\0")
  )
    return false;
  if (typeof params.offset === "number" && params.offset > 0) {
    if (type === "RepositoryExploreFilesParams" && params.version === null) return false;
    if (
      ["RepositoryExploreContentParams", "RepositoryExploreDiffParams"].includes(type) &&
      params.contentSha256 === null
    )
      return false;
  }
  return true;
}
export function validExploreResult(
  type: string,
  result: Record<string, unknown>,
  params: object,
): boolean {
  if (
    Buffer.byteLength(JSON.stringify(result)) > 1024 * 1024 - 1024 ||
    !validPublicationDTO(type, result) ||
    !sameLocalValue(result.request, params)
  )
    return false;
  const request = params as Record<string, unknown>;
  const offset = request.offset as number;
  const next = result.nextOffset;
  if (type === "RepositoryExploreInfo") {
    return sameLocalValue(
      (result.context as Record<string, unknown>).repository,
      request.repository,
    );
  }
  if (type === "RepositoryExploreFilesPage") {
    const files = result.files as Array<{ path: string; oldPath: string | null; status: string }>;
    const total = result.total as number;
    return (
      (request.version === null || result.version === request.version) &&
      offset <= total &&
      files.length === Math.min(request.limit as number, total - offset) &&
      new Set(files.map((file) => file.path)).size === files.length &&
      files.every(
        (file, index) =>
          path(file.path) &&
          (file.oldPath === null || path(file.oldPath)) &&
          (!request.changedOnly || file.status !== "unchanged") &&
          (!(request.query as string).trim() ||
            file.path.toLowerCase().includes((request.query as string).trim().toLowerCase())) &&
          (index === 0 ||
            Buffer.compare(Buffer.from(files[index - 1]!.path), Buffer.from(file.path)) < 0),
      ) &&
      next === (offset + files.length < total ? offset + files.length : null)
    );
  }
  if (["RepositoryExploreContentPage", "RepositoryExploreDiffPage"].includes(type)) {
    const text = result.text as string;
    const bytes = Buffer.byteLength(text);
    if (
      bytes > (request.maxBytes as number) ||
      (request.contentSha256 !== null && request.contentSha256 !== result.contentSha256)
    )
      return false;
    if (type === "RepositoryExploreContentPage" && result.state !== "text") {
      if (text !== "" || next !== null || offset !== 0) return false;
      if (result.state === "binary")
        return (
          result.contentSha256 !== null &&
          typeof result.size === "number" &&
          result.size <= 512 * 1024
        );
      if (result.state === "tooLarge")
        return (
          result.contentSha256 === null &&
          typeof result.size === "number" &&
          result.size > 512 * 1024
        );
      return result.contentSha256 === null && result.size === null;
    }
    const size = (
      type === "RepositoryExploreContentPage" ? result.size : result.totalBytes
    ) as number;
    return (
      typeof size === "number" &&
      (type !== "RepositoryExploreContentPage" ||
        (size <= 512 * 1024 && result.contentSha256 !== null)) &&
      offset + bytes <= size &&
      (next === null
        ? offset + bytes === size
        : bytes > 0 && next === offset + bytes && next < size)
    );
  }
  if (type === "RepositoryExploreBlamePage") {
    const lines = result.lines as Array<{ line: number; sha: string; shortSha: string }>;
    const total = result.totalLines as number;
    if (offset > total) return false;
    if (!result.available) return lines.length === 0 && next === null;
    const end = offset + lines.length;
    return (
      lines.length <= (request.limit as number) &&
      end <= total &&
      (offset === total || lines.length > 0) &&
      lines.every(
        (line, index) => line.line === offset + index + 1 && line.shortSha === line.sha.slice(0, 8),
      ) &&
      next === (end < total ? end : null)
    );
  }
  if (type === "RepositoryExploreFindPage") {
    const matches = result.matches as Array<{ startUtf16: number; endUtf16: number }>;
    const total = result.total as number;
    const end = offset + matches.length;
    return (
      offset <= total &&
      matches.length === Math.min(request.limit as number, total - offset) &&
      matches.every(
        (match, index) =>
          match.endUtf16 > match.startUtf16 &&
          (index === 0 || matches[index - 1]!.startUtf16 <= match.startUtf16),
      ) &&
      next === (end < total ? end : null)
    );
  }
  return false;
}
