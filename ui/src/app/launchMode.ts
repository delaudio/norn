export function launchMode(args: string[]): "current-repo" | "workspace" {
  for (const arg of args) {
    if (arg !== "--current-repo" && arg !== "--workspace")
      throw new Error(`Unknown argument: ${arg}. Use --help for usage.`);
  }
  if (args.includes("--current-repo") && args.includes("--workspace"))
    throw new Error("Choose either --current-repo or --workspace.");
  return args.includes("--workspace") ? "workspace" : "current-repo";
}
