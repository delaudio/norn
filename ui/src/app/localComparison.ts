import type { LocalComparison, LocalSource } from "../../../src/protocol/generated/protocol";
import type { RepositorySummary } from "../transport/backendClient";
import type { LocalLayer, LocalSourceOptions } from "../transport/localEvidence";
import { sameLocalValue } from "../transport/localValidation";
export const LOCAL_LAYERS: LocalLayer[] = ["committed", "staged", "unstaged"];
export interface LocalComparisonForm {
  sources: Array<{ label: string; source: LocalSource }>;
  sourceIndex: number;
  baseIndex: number;
  layers: LocalLayer[];
  layerCursor: number;
  focus: "source" | "base" | "layers";
  options: LocalSourceOptions | null;
  loading: boolean;
  capturing: boolean;
  error: string | null;
}
export function localComparisonForm(
  repositories: RepositorySummary[],
  selected?: LocalSource,
): LocalComparisonForm {
  const sources: LocalComparisonForm["sources"] = [
    { label: "Current repository", source: { kind: "current" } },
  ];
  for (const repository of repositories) {
    if (!repository.localPath) continue;
    const source: LocalSource = {
      kind: "configured",
      repository: {
        provider: repository.provider,
        workspace: repository.workspace,
        repo: repository.repo,
      },
    };
    if (!sources.some((entry) => sameLocalValue(entry.source, source)))
      sources.push({
        label: `${repository.provider} ${repository.workspace}/${repository.repo}`,
        source,
      });
  }
  return {
    sources,
    sourceIndex: Math.max(
      0,
      sources.findIndex((entry) => sameLocalValue(entry.source, selected)),
    ),
    baseIndex: 0,
    layers: [...LOCAL_LAYERS],
    layerCursor: 0,
    focus: "source",
    options: null,
    loading: true,
    capturing: false,
    error: null,
  };
}

export function selectedLocalLayers(
  layers: LocalLayer[],
): Extract<LocalComparison, { mode: "workingTree" }>["layers"] {
  const [first, second, third] = layers;
  if (!first || layers.length > 3) throw new Error("Select between one and three local layers.");
  if (third && second) return [first, second, third];
  if (second) return [first, second];
  return [first];
}
