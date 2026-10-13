import optionsFixture from "../../../protocol/fixtures/valid/response-local-options.json";
import { FakeProviderTransport } from "../../src/transport/fakeProviderTransport";

export class CurrentTransport extends FakeProviderTransport {
  failure = false;
  constructor(readonly provider: "github" | "bitbucket") {
    super();
  }
  override async start() {
    const ready = await super.start();
    return {
      ...ready,
      capabilities: [...ready.capabilities, "local-snapshots", "local-evidence-reviews"],
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
    return super.request(method, params);
  }
}
