import { test } from "node:test";
import assert from "node:assert/strict";
import { MagpieClient } from "../src/client.ts";

test("does not submit a saved secret to an older service that would ignore auth_mode", async () => {
  const calls: { url: string; body: unknown }[] = [];
  const client = new MagpieClient({
    apiKey: "test-admin",
    fetch: async (url, init) => {
      calls.push({ url: String(url), body: init?.body });
      return Response.json({ version: "0.1.2" });
    },
  });
  await assert.rejects(
    client.connectProvider({ kind: "claude_code", auth_mode: "saved_token", oauth_token: "test-secret" }),
    /Restart the harness/,
  );
  assert.equal(calls.length, 1);
  assert.ok(calls[0].url.endsWith("/v1/status"));
  assert.equal(calls[0].body, undefined);
});

test("sends saved profile credentials only after capability negotiation", async () => {
  const calls: string[] = [];
  const client = new MagpieClient({
    apiKey: "test-admin",
    fetch: async (url, init) => {
      calls.push(String(url));
      if (String(url).endsWith("/v1/status")) return Response.json({ features: ["auth_profiles"] });
      assert.equal(init?.method, "POST");
      assert.equal(JSON.parse(String(init?.body)).auth_mode, "isolated");
      return Response.json({ id: "test-profile", status: "needs_auth" });
    },
  });
  const profile = await client.connectProvider({ kind: "codex_cli", auth_mode: "isolated" });
  assert.equal(profile.status, "needs_auth");
  assert.equal(calls.length, 2);
});
