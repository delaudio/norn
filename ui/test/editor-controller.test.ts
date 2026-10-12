import { expect, test } from "bun:test";
import type {
  RepositoryEditor,
  RepositoryEditorPrepareParams,
} from "../../src/protocol/generated/protocol";
import { EditorController } from "../src/app/editorController";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";
import { BackendRequestError } from "../src/transport/types";

const context = {
  repository: { provider: "github", workspace: "owned", repo: "editor" },
  rootId: "a".repeat(64),
} as const;
const params: RepositoryEditorPrepareParams = {
  context,
  path: "src/--owned ü.txt",
  line: 7,
  mode: "terminal",
};
const editor = (state: RepositoryEditor["state"]): RepositoryEditor => ({
  ...params,
  launchId: "b".repeat(64),
  integration: "nvim",
  state,
  exitCode: state === "exited" ? 0 : null,
  message: null,
});
class Fixture extends FakeBackendTransport {
  calls: string[] = [];
  gate: Promise<void> | null = null;
  refusal = false;
  malformed = false;
  running = false;
  unavailable = false;
  graphical = false;
  override async request(method: string, request: Record<string, unknown>) {
    this.calls.push(method);
    if (method === "repository.editor.prepare") {
      if (this.gate) await this.gate;
      return {
        request,
        token: this.unavailable ? null : "c".repeat(64),
        integration: this.unavailable ? null : "nvim",
        expiresInMs: this.unavailable ? 0 : 30000,
        message: this.unavailable ? "Configure an editor." : null,
      };
    }
    if (method === "repository.editor.open") {
      if (this.refusal) throw new BackendRequestError("conflict", "Expired action.");
      return {
        request,
        editor: this.graphical
          ? { ...editor("launched"), mode: "graphical" }
          : editor(this.running ? "running" : "exited"),
      };
    }
    if (method === "repository.editor.status")
      return {
        request,
        editor: this.malformed
          ? { ...editor("exited"), launchId: "d".repeat(64) }
          : editor(this.running ? "running" : "exited"),
      };
    if (method === "repository.editor.cancel") {
      this.running = false;
      return { request, editor: editor("canceled") };
    }
    return super.request(method, request);
  }
  override close() {
    this.calls.push("close");
    super.close();
  }
}
function owned() {
  const fixture = new Fixture();
  const events: string[] = [];
  const controller = new EditorController(new BackendClient(fixture), {
    isDestroyed: false,
    suspend: () => events.push("suspend"),
    resume: () => events.push("resume"),
  });
  controller.resetScope("owned");
  return { fixture, events, controller };
}
async function until(condition: () => boolean) {
  const deadline = Date.now() + 1500;
  while (!condition()) {
    if (Date.now() > deadline) throw new Error("Editor state deadline");
    await Bun.sleep(5);
  }
}
test("terminal ownership suspends before opening and resumes once after exit", async () => {
  const { fixture, events, controller } = owned();
  await controller.start(params);
  expect(events).toEqual(["suspend", "resume"]);
  expect(controller.getSnapshot().editor?.state).toBe("exited");
  expect(controller.getSnapshot().busy).toBe(false);
  expect(fixture.calls).toEqual(["repository.editor.prepare", "repository.editor.open"]);
});
test("late prepare after repository change cannot launch", async () => {
  const { fixture, events, controller } = owned();
  let release!: () => void;
  fixture.gate = new Promise<void>((r) => {
    release = r;
  });
  const task = controller.start(params);
  controller.resetScope("another-root");
  release();
  await task;
  expect(events).toEqual([]);
  expect(fixture.calls).toEqual(["repository.editor.prepare"]);
  expect(controller.getSnapshot().busy).toBe(false);
});
test("validated refusal resumes and keeps backend usable", async () => {
  const { fixture, events, controller } = owned();
  fixture.refusal = true;
  await controller.start(params);
  expect(events).toEqual(["suspend", "resume"]);
  expect(fixture.calls).not.toContain("close");
  expect(controller.getSnapshot().error).toBe("Expired action.");
});
test("malformed status closes owner before resuming", async () => {
  const { fixture, events, controller } = owned();
  fixture.running = true;
  fixture.malformed = true;
  await controller.start(params);
  expect(fixture.calls).toContain("close");
  expect(events).toEqual(["suspend", "resume"]);
  expect(controller.getSnapshot().error).not.toBeNull();
});
test("scope change cancels active editor and awaits final state", async () => {
  const { fixture, events, controller } = owned();
  fixture.running = true;
  const task = controller.start(params);
  await until(() => controller.getSnapshot().editor?.state === "running");
  controller.resetScope("another-root");
  await task;
  expect(fixture.calls.filter((m) => m === "repository.editor.cancel")).toHaveLength(1);
  expect(events).toEqual(["suspend", "resume"]);
  expect(controller.getSnapshot().editor).toBeNull();
  expect(controller.getSnapshot().busy).toBe(false);
});
test("unavailable integration never transfers ownership", async () => {
  const { fixture, events, controller } = owned();
  fixture.unavailable = true;
  await controller.start(params);
  expect(events).toEqual([]);
  expect(controller.getSnapshot().message).toBe("Configure an editor.");
});

test("graphical launch never suspends terminal input or waits for the window to close", async () => {
  const { fixture, events, controller } = owned();
  fixture.graphical = true;
  await controller.start({ ...params, mode: "graphical" });
  expect(events).toEqual([]);
  expect(controller.getSnapshot().editor?.state).toBe("launched");
  expect(fixture.calls).not.toContain("repository.editor.status");
  expect(controller.getSnapshot().busy).toBe(false);
});
