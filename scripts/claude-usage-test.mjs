/** Test the real native status-line helper with an explicit local fixture. */
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const binary = resolve(
  process.env.MAGPIE_TEST_DESKTOP || `target/release/magpie-desktop${process.platform === "win32" ? ".exe" : ""}`,
);
const root = await mkdtemp(join(tmpdir(), "magpie usage test "));
const binding = {
  account_id: "fixture-account",
  settings_path: join(root, "settings.json"),
  command: "fixture",
  previous: { type: "command", command: "echo preserved-status" },
};
const expiry = Math.floor(Date.now() / 1000) + 3600;
const payload = {
  rate_limits: {
    five_hour: { used_percentage: 0.5, resets_at: expiry },
    seven_day: { used_percentage: 23, resets_at: expiry + 86400 },
  },
  transcript_path: "must-not-store",
  api_key: "must-not-store",
};
async function invoke(data) {
  const env = { ...process.env, MAGPIE_HOME: root };
  delete env.DISPLAY;
  delete env.WAYLAND_DISPLAY;
  const p = spawn(binary, ["--claude-statusline", root], { env, stdio: ["pipe", "pipe", "pipe"], timeout: 15000 });
  let stdout = "",
    stderr = "";
  p.stdout.on("data", (d) => (stdout += d));
  p.stderr.on("data", (d) => (stderr += d));
  p.stdin.end(JSON.stringify(data));
  const code = await new Promise((ok, fail) => {
    p.once("error", fail);
    p.once("close", ok);
  });
  assert.equal(code, 0, stderr);
  return stdout.trim();
}
try {
  await writeFile(join(root, "claude-statusline.json"), JSON.stringify(binding));
  assert.equal(await invoke(payload), "preserved-status");
  const text = await readFile(join(root, "claude-usage.json"), "utf8");
  const snapshot = JSON.parse(text);
  assert.equal(snapshot.account_id, "fixture-account");
  assert.equal(snapshot.rate_limits.five_hour.used_percentage, 0.5);
  assert.equal(snapshot.rate_limits.five_hour.resets_at, expiry);
  assert.equal(snapshot.rate_limits.seven_day.used_percentage, 23);
  assert.ok(!text.includes("must-not-store"));
  assert.equal(await invoke({ rate_limits: null }), "preserved-status");
  assert.equal(
    await readFile(join(root, "claude-usage.json"), "utf8"),
    text,
    "Missing data must not fabricate zero usage",
  );
  binding.previous = null;
  await writeFile(join(root, "claude-statusline.json"), JSON.stringify(binding));
  assert.match(await invoke(payload), /Weekly: 23% used/);
  console.log(
    "PASS: native Claude usage helper, reported percentages/resets, private field filtering, existing status-line output, absent allowance handling",
  );
} finally {
  await rm(root, { recursive: true, force: true });
}
