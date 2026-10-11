import { parseDiffSyntax } from "../app/syntaxTokens";

process.on("message", (message: unknown) => {
  const request = message as { path?: unknown; diff?: unknown };
  if (
    !request ||
    typeof request.path !== "string" ||
    typeof request.diff !== "string" ||
    request.path.length > 4096 ||
    Buffer.byteLength(request.diff) > 128 * 1024
  )
    process.exit(2);
  process.send?.(parseDiffSyntax(request.path, request.diff));
});
process.on("disconnect", () => process.exit(0));
