// Isolated test-only provider. Never imported by the application.
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { readFile, mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const root = resolve("../..");
const data = await mkdtemp(join(tmpdir(), "magpie-e2e-"));
const infoFile = resolve(".e2e-connection.json");
const children = [];
const fixture = createServer((req, res) => {
  let body = "";
  req.on("data", (chunk) => (body += chunk));
  req.on("end", () => {
    if (req.url === "/v1/models") {
      res.setHeader("Content-Type", "application/json");
      res.end(
        JSON.stringify({
          data: [
            {
              id: "magpie-test-fixture",
              name: "Test fixture model",
              context_length: 32768,
            },
          ],
        }),
      );
    } else if (req.url === "/v1/chat/completions") {
      res.writeHead(200, {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
      });
      const chunks = [
        {
          id: "fixture",
          choices: [
            {
              index: 0,
              delta: { content: "Test fixture response." },
              finish_reason: null,
            },
          ],
        },
        {
          id: "fixture",
          choices: [{ index: 0, delta: {}, finish_reason: "stop" }],
          usage: { prompt_tokens: 12, completion_tokens: 5, total_tokens: 17 },
        },
      ];
      for (const chunk of chunks)
        res.write(`data: ${JSON.stringify(chunk)}\n\n`);
      res.end("data: [DONE]\n\n");
    } else {
      res.writeHead(404);
      res.end();
    }
  });
});
await new Promise((ok) => fixture.listen(17879, "127.0.0.1", ok));
const binary =
  process.env.MAGPIE_TEST_BINARY || join(root, "target/debug/magpie");
const harness = spawn(binary, ["serve", "--port", "17878"], {
  cwd: root,
  env: { ...process.env, MAGPIE_HOME: data, MAGPIE_SECRET_STORE: "file" },
  stdio: ["ignore", "ignore", "inherit"],
});
children.push(harness);
harness.on("error", (e) => {
  console.error(e.message);
  process.exit(1);
});
let token;
for (let i = 0; i < 100; i++) {
  try {
    const r = await fetch("http://127.0.0.1:17878/health");
    if (r.ok) {
      token = (await readFile(join(data, "admin.token"), "utf8")).trim();
      break;
    }
  } catch {}
  await new Promise((r) => setTimeout(r, 200));
}
if (!token) throw new Error("Test harness did not start");
await writeFile(
  infoFile,
  JSON.stringify({ token, url: "http://127.0.0.1:17878", data }),
  { mode: 0o600 },
);
const vite = spawn("pnpm", ["dev"], {
  env: {
    ...process.env,
    VITE_MAGPIE_TOKEN: token,
    VITE_MAGPIE_URL: "http://127.0.0.1:17878",
    MAGPIE_EXTERNAL_STORE: "1",
  },
  stdio: "inherit",
});
children.push(vite);
let stopping = false;
async function stop() {
  if (stopping) return;
  stopping = true;
  for (const p of children) p.kill("SIGTERM");
  fixture.closeAllConnections();
  fixture.close();
  await rm(infoFile, { force: true });
  // Test data is disposable and intentionally separate from the user's data.
  await rm(data, { recursive: true, force: true });
  process.exit(0);
}
process.on("SIGTERM", stop);
process.on("SIGINT", stop);
