// Test-only, fixed owned bare repository. Never resolves or forwards provider hosts.
import assert from "node:assert/strict";
import { join } from "node:path";

export function ownedGitProxy(git: string, directory: string, environment: Record<string, string>) {
  const exec = Bun.spawnSync([git, "--exec-path"], {
    env: environment,
    stdout: "pipe",
    stderr: "pipe",
  });
  assert.equal(exec.exitCode, 0);
  const executable = join(
    exec.stdout.toString().trim(),
    process.platform === "win32" ? "git-http-backend.exe" : "git-http-backend",
  );
  const children = new Set<ReturnType<typeof Bun.spawn>>();
  const requests: string[] = [];
  let reject = false;
  let delay = false;
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    async fetch(request) {
      const url = new URL(request.url);
      const host = request.headers.get("host")?.split(":")[0];
      if (reject) return new Response("Owned failure", { status: 503 });
      if (
        !host ||
        !["github.com", "bitbucket.org"].includes(host) ||
        !["/owned/operations.git/info/refs", "/owned/operations.git/git-upload-pack"].includes(
          url.pathname,
        ) ||
        (request.method !== "GET" && request.method !== "POST") ||
        (url.pathname.endsWith("/info/refs") &&
          (request.method !== "GET" || url.search !== "?service=git-upload-pack")) ||
        (url.pathname.endsWith("/git-upload-pack") &&
          (request.method !== "POST" || url.search !== ""))
      )
        return new Response("Unregistered request", { status: 403 });
      if (requests.length >= 100) return new Response("Owned request limit", { status: 429 });
      requests.push(`${host}${url.pathname}`);
      if (delay) await Bun.sleep(2000);
      if (Number(request.headers.get("content-length") ?? 0) > 4 * 1024 * 1024)
        return new Response("Too large", { status: 413 });
      const body = new Uint8Array(await request.arrayBuffer());
      if (body.byteLength > 4 * 1024 * 1024) return new Response("Too large", { status: 413 });
      const child = Bun.spawn([executable], {
        env: {
          ...environment,
          GIT_PROJECT_ROOT: directory,
          GIT_HTTP_EXPORT_ALL: "1",
          PATH_INFO: `/remote.git${url.pathname.slice("/owned/operations.git".length)}`,
          QUERY_STRING: url.search.slice(1),
          REQUEST_METHOD: request.method,
          CONTENT_TYPE: request.headers.get("content-type") ?? "",
          CONTENT_LENGTH: String(body.length),
          SERVER_PROTOCOL: "HTTP/1.1",
          REMOTE_ADDR: "127.0.0.1",
          HTTP_GIT_PROTOCOL: request.headers.get("git-protocol") ?? "",
        },
        stdin: "pipe",
        stdout: "pipe",
        stderr: "pipe",
      });
      children.add(child);
      const timer = setTimeout(() => child.kill(), 10000);
      const bounded = async (stream: ReadableStream<Uint8Array>, limit: number) => {
        const reader = stream.getReader();
        const parts: Uint8Array[] = [];
        let size = 0;
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          size += value.length;
          if (size > limit) {
            child.kill();
            throw new Error("Owned CGI output limit");
          }
          parts.push(value);
        }
        return Buffer.concat(parts);
      };
      try {
        const outputs = Promise.all([
          bounded(child.stdout, 8 * 1024 * 1024),
          bounded(child.stderr, 128 * 1024),
        ]);
        child.stdin.write(body);
        await child.stdin.flush();
        child.stdin.end();
        const [output] = await outputs;
        assert.equal(await child.exited, 0);
        const split = output.indexOf("\r\n\r\n");
        assert(split >= 0);
        const headers = new Headers();
        let status = 200;
        for (const line of output.subarray(0, split).toString().split("\r\n")) {
          const colon = line.indexOf(":");
          assert(colon > 0);
          const key = line.slice(0, colon),
            value = line.slice(colon + 1).trim();
          if (key.toLowerCase() === "status") status = Number.parseInt(value, 10);
          else if (
            !["content-length", "connection", "transfer-encoding"].includes(key.toLowerCase())
          )
            headers.set(key, value);
        }
        return new Response(output.subarray(split + 4), { status, headers });
      } catch {
        return new Response("Owned CGI failed", { status: 502 });
      } finally {
        clearTimeout(timer);
        if (child.exitCode === null) child.kill();
        await child.exited;
        children.delete(child);
      }
    },
  });
  return {
    url: `http://127.0.0.1:${server.port}`,
    requests,
    fail(value: boolean) {
      reject = value;
    },
    delay(value: boolean) {
      delay = value;
    },
    async close() {
      server.stop(true);
      for (const child of children) child.kill();
      await Promise.all([...children].map((child) => child.exited));
    },
  };
}
