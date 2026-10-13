import optionsFixture from "../../../protocol/fixtures/valid/response-local-options.json";
import { FakeProviderTransport } from "../../src/transport/fakeProviderTransport";
import { LocalTransport } from "./local-backend";

export class CurrentTransport extends FakeProviderTransport {
  failure = false;
  private local = new LocalTransport();
  constructor(readonly provider: "github" | "bitbucket") {
    super();
  }
  override async start() {
    const ready = await super.start();
    return {
      ...ready,
      capabilities: [
        ...ready.capabilities,
        "local-snapshots",
        "local-evidence-reviews",
        "provider-evidence-reviews",
        "repository-explorer-reads",
        "repository-explorer-windowed-find",
      ],
    };
  }
  override async request(method: string, params: Record<string, unknown>) {
    if (method === "local.options") {
      this.calls.push({ method, params });
      if (this.failure) throw new Error("No supported Git remote in the current directory");
      return {
        ...structuredClone(optionsFixture.result),
        request: params,
        repository: { provider: this.provider, workspace: "synthetic", repo: "review" },
      };
    }
    if (method.startsWith("local.")) {
      this.calls.push({ method, params });
      const result = await this.local.request(method, params);
      const qualify = (value: unknown): unknown => {
        if (Array.isArray(value)) return value.map(qualify);
        if (value && typeof value === "object") {
          const record = value as Record<string, unknown>;
          if (record.workspace === "owned" && record.repo === "same" && "provider" in record)
            return { provider: this.provider, workspace: "synthetic", repo: "review" };
          return Object.fromEntries(
            Object.entries(record).map(([key, child]) => [key, qualify(child)]),
          );
        }
        return value;
      };
      return qualify(result) as Record<string, unknown>;
    }
    return super.request(method, params);
  }
}
