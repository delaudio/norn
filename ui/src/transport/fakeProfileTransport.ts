import { FakeBackendTransport, type FakeTransportOptions } from "./fakeTransport";

/** Owned policy fixtures only: no repository files, credentials or network. */
export class FakeProfileTransport extends FakeBackendTransport {
  calls: Array<{ method: string; params: Record<string, unknown> }> = [];
  profiles = ["default", "security", "strict"];
  defaultProfile: string | null = "default";
  source: "policy" | "base" = "policy";
  notice: string | null = null;
  error: string | null = null;
  gate: Promise<void> | null = null;
  mutate: ((value: Record<string, unknown>) => Record<string, unknown>) | null = null;
  constructor(options: FakeTransportOptions = {}) {
    super(options);
  }
  async start() {
    const ready = await super.start();
    return { ...ready, capabilities: [...ready.capabilities, "review-profile-catalog"] };
  }
  async request(method: string, params: Record<string, unknown>): Promise<Record<string, unknown>> {
    this.calls.push({ method, params: structuredClone(params) });
    if (method !== "review.profiles") return super.request(method, params);
    if (this.gate) await this.gate;
    if (this.error) throw new Error(this.error);
    const result = {
      target: structuredClone(params.target),
      profiles: [...this.profiles],
      defaultProfile: this.defaultProfile,
      source: this.source,
      notice: this.notice,
    };
    return this.mutate ? this.mutate(result) : result;
  }
}
