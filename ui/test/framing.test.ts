import { expect, test } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { readProtocolLines } from "../src/transport/framing";
import { MAX_FRAME_BYTES, parseFrame } from "../src/transport/protocol";

async function collect(chunks: Uint8Array[]) {
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      for (const chunk of chunks) controller.enqueue(chunk);
      controller.close();
    },
  });
  const lines: string[] = [];
  for await (const line of readProtocolLines(stream)) lines.push(line);
  return lines;
}
const encoder = new TextEncoder();
test("wire framing preserves fragmented UTF-8 and multiple complete frames", async () => {
  const bytes = encoder.encode('"ü😀"\r\n{}\n');
  const lines = await collect(Array.from(bytes, (byte) => new Uint8Array([byte])));
  expect(lines).toEqual(['"ü😀"\r', "{}"]);
});
test("wire framing bounds terminated and unterminated frames by bytes", async () => {
  expect(await collect([encoder.encode(`${"a".repeat(MAX_FRAME_BYTES - 1)}\n`)])).toHaveLength(1);
  await expect(collect([encoder.encode(`${"a".repeat(MAX_FRAME_BYTES)}\n`)])).rejects.toThrow(
    "byte limit",
  );
  await expect(collect([new Uint8Array(MAX_FRAME_BYTES), new Uint8Array([65])])).rejects.toThrow(
    "byte limit",
  );
  await expect(collect([encoder.encode("😀".repeat(MAX_FRAME_BYTES / 4 + 1))])).rejects.toThrow(
    "byte limit",
  );
});
test("wire framing rejects invalid UTF-8 and incomplete EOF", async () => {
  await expect(collect([new Uint8Array([255, 10])])).rejects.toThrow("UTF-8");
  await expect(collect([encoder.encode("{}")])).rejects.toThrow("incomplete");
});
test("client frames use the complete generated schema including forbidden extra fields", () => {
  expect(parseFrame('{"type":"event"}')).toBeNull();
  expect(
    parseFrame('{"type":"response","id":"x","ok":true,"result":{},"secret":"owned"}'),
  ).toBeNull();
  expect(
    parseFrame('{"type":"ready","protocolVersion":1,"serverVersion":"owned","capabilities":[]}')
      ?.type,
  ).toBe("ready");
});

test("client accepts every shared valid wire fixture and rejects all invalid ones", async () => {
  for (const kind of ["valid", "invalid"]) {
    const directory = new URL(`../../protocol/fixtures/${kind}/`, import.meta.url);
    for (const file of await readdir(directory)) {
      if (!file.endsWith(".json")) continue;
      const frame = parseFrame(await readFile(new URL(file, directory), "utf8"));
      expect(frame !== null, `${kind}/${file}`).toBe(kind === "valid");
    }
  }
});
