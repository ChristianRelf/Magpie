/** Isolated lifecycle/security smoke test. Does not call a real AI provider. */
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const cli = resolve(
  process.env.MAGPIE_TEST_BINARY || `target/debug/magpie${process.platform === "win32" ? ".exe" : ""}`,
);
const desktop = process.env.MAGPIE_TEST_DESKTOP && resolve(process.env.MAGPIE_TEST_DESKTOP);
const home = await mkdtemp(join(tmpdir(), "magpie-lifecycle-"));
const env = { ...process.env, MAGPIE_HOME: home };
delete env.MAGPIE_API_KEY;
delete env.MAGPIE_URL;
delete env.DISPLAY;
delete env.WAYLAND_DISPLAY;
const children = [];
function launch(bin, args, additions = {}) {
  const p = spawn(bin, args, { env: { ...env, ...additions }, stdio: ["ignore", "pipe", "pipe"] });
  children.push(p);
  p.output = "";
  p.stdout.on("data", (d) => (p.output += d));
  p.stderr.on("data", (d) => (p.output += d));
  p.finished = new Promise((ok, fail) => {
    p.once("error", fail);
    p.once("exit", ok);
  });
  return p;
}
async function finished(p) {
  return Promise.race([
    p.finished,
    delay(15000, undefined, { ref: false }).then(() => {
      throw Error("Process did not exit: " + p.output);
    }),
  ]);
}
const socket = createServer();
await new Promise((ok) => socket.listen(0, "127.0.0.1", ok));
const port = socket.address().port;
await new Promise((ok) => socket.close(ok));
const url = `http://127.0.0.1:${port}`;
async function ready() {
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`${url}/health`)).ok) return;
    } catch {}
    await delay(100);
  }
  throw Error("Harness did not become healthy");
}
let token;
async function api(path, method = "GET", body, key = token) {
  return fetch(url + path, {
    method,
    headers: { Authorization: `Bearer ${key}`, "Content-Type": "application/json" },
    body: body && JSON.stringify(body),
  });
}
try {
  const first = launch(cli, ["serve", "--port", String(port)]);
  await ready();
  token = (await readFile(join(home, "admin.token"), "utf8")).trim();
  assert.equal((await fetch(url + "/v1/status")).status, 401);
  const settings = (await (await api("/v1/settings")).json()).settings;
  settings.general.theme = "light";
  assert.equal((await api("/v1/settings", "PUT", settings)).status, 200);
  const key = await (await api("/v1/keys", "POST", { name: "Lifecycle read-only", scopes: ["read"] })).json();
  const scoped = { MAGPIE_API_KEY: key.token, MAGPIE_URL: url };
  assert.equal(await finished(launch(cli, ["--no-start", "status"], scoped)), 0);
  assert.notEqual(await finished(launch(cli, ["stop"], scoped)), 0);
  assert.equal((await api("/v1/status")).status, 200);
  assert.equal((await api("/v1/admin/shutdown", "POST", {})).status, 200);
  assert.equal(await finished(first), 0);
  const binary = desktop || cli;
  const args = desktop ? ["--harness"] : ["serve"];
  const second = launch(binary, args);
  await ready();
  assert.equal((await (await api("/v1/settings")).json()).settings.general.theme, "light");
  const duplicate = launch(binary, args);
  assert.notEqual(await finished(duplicate), 0, "Duplicate harness must not acquire the data lock");
  assert.equal((await api("/v1/admin/shutdown", "POST", {})).status, 200);
  assert.equal(await finished(second), 0);
  console.log(
    `PASS: authenticated API, scoped CLI, durable settings, duplicate-process lock, graceful shutdown${desktop ? ", native daemon without a display" : ""}`,
  );
} finally {
  for (const p of children) if (p.exitCode === null && !p.killed) p.kill();
  await Promise.allSettled(children.map((p) => p.finished));
  await rm(home, { recursive: true, force: true });
}
