import { test } from "node:test";
import assert from "node:assert/strict";
import { parseSSE } from "../src/client.ts";

function streamOf(chunks: string[]): ReadableStream<Uint8Array> {
  const enc = new TextEncoder();
  return new ReadableStream({
    start(c) {
      for (const ch of chunks) c.enqueue(enc.encode(ch));
      c.close();
    },
  });
}

test("parses events split across chunks", async () => {
  const out = [];
  for await (const ev of parseSSE(
    streamOf([
      'event: started\ndata: {"a"',
      ":1}\n\n: keep-alive\n\ndata: x\r\n\r\n",
    ]),
  ))
    out.push(ev);
  assert.deepEqual(out, [
    { event: "started", data: '{"a":1}' },
    { event: undefined, data: "x" },
  ]);
});

test("handles every CRLF byte boundary without dropping multiline data", async () => {
  const input = "event: delta\r\ndata: first\r\ndata: second\r\n\r\n";
  for (let i = 1; i < input.length; i++) {
    const out = [];
    for await (const event of parseSSE(
      streamOf([input.slice(0, i), input.slice(i)]),
    ))
      out.push(event);
    assert.deepEqual(
      out,
      [{ event: "delta", data: "first\nsecond" }],
      `split at ${i}`,
    );
  }
});

test("stopping consumption cancels the underlying response", async () => {
  let cancelled = false;
  const body = new ReadableStream<Uint8Array>({
    start(c) {
      c.enqueue(new TextEncoder().encode("data: first\n\n"));
    },
    cancel() {
      cancelled = true;
    },
  });
  for await (const _event of parseSSE(body)) break;
  assert.equal(cancelled, true);
});

test("abort cancels a reader blocked waiting for the next chunk", async () => {
  let cancelled = false;
  const ctrl = new AbortController();
  const body = new ReadableStream<Uint8Array>({
    cancel() {
      cancelled = true;
    },
  });
  const task = (async () => {
    for await (const _event of parseSSE(body, ctrl.signal)) {
      /* empty */
    }
  })();
  ctrl.abort();
  await task;
  assert.equal(cancelled, true);
});
