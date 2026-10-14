import { MAX_FRAME_BYTES } from "./protocol";

// Count wire bytes before decoding or concatenating. A missing newline must
// never permit unbounded retained output. Each accepted frame is valid UTF-8.
export async function* readProtocolLines(
  stream: ReadableStream<Uint8Array>,
): AsyncGenerator<string> {
  const reader = stream.getReader();
  let decoder = new TextDecoder("utf-8", { fatal: true });
  let buffer = "";
  let bytes = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        if (bytes !== 0) throw new Error("backend protocol ended in an incomplete frame");
        return;
      }
      let offset = 0;
      while (offset < value.length) {
        const newline = value.indexOf(10, offset);
        const end = newline < 0 ? value.length : newline;
        const size = end - offset;
        if (bytes + size + (newline < 0 ? 0 : 1) > MAX_FRAME_BYTES)
          throw new Error("backend protocol frame exceeds the byte limit");
        bytes += size;
        try {
          buffer += decoder.decode(value.subarray(offset, end), { stream: newline < 0 });
        } catch {
          throw new Error("backend protocol frame is not valid UTF-8");
        }
        if (newline < 0) break;
        yield buffer;
        buffer = "";
        bytes = 0;
        decoder = new TextDecoder("utf-8", { fatal: true });
        offset = newline + 1;
      }
    }
  } finally {
    reader.releaseLock();
  }
}
