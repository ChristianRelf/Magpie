#!/usr/bin/env node
// Test-only official-protocol fixture. Never packaged with the application.
const fs = require("node:fs");
const path = require("node:path");
const readline = require("node:readline");
const args = process.argv.slice(2);
if (args.includes("--version")) {
  console.log("authentication profile test fixture");
  process.exit(0);
}
if (args.includes("status")) {
  if (!process.env.CLAUDE_CONFIG_DIR || !process.env.CLAUDE_CODE_OAUTH_TOKEN) process.exit(1);
  console.log(JSON.stringify({ loggedIn: true, authMethod: "oauth_token", apiProvider: "firstParty" }));
  process.exit(0);
}
if (!args.includes('cli_auth_credentials_store="keyring"') || !process.env.CODEX_HOME) process.exit(1);
const marker = path.join(process.env.CODEX_HOME, "fixture-identity");
let pending;
const send = (value) => process.stdout.write(JSON.stringify(value) + "\n");
readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const message = JSON.parse(line);
  if (message.id === undefined) return;
  let result = {};
  switch (message.method) {
    case "account/login/start":
      result = { loginId: "fixture", authUrl: "https://auth.openai.com/test-fixture" };
      pending = setTimeout(() => {
        fs.writeFileSync(marker, "Fixture identity, never a credential");
        send({ method: "account/login/completed", params: { loginId: "fixture", success: true, error: null } });
      }, 3000);
      break;
    case "account/login/cancel":
      clearTimeout(pending);
      break;
    case "account/logout":
      clearTimeout(pending);
      fs.rmSync(marker, { force: true });
      break;
    case "account/read":
      result = {
        account: fs.existsSync(marker)
          ? { type: "chatgpt", email: `${path.basename(process.env.CODEX_HOME)}@example.test`, planType: "plus" }
          : null,
      };
      break;
    case "model/list":
      result = { data: [{ model: "codex-fixture", displayName: "Codex fixture model" }], nextCursor: null };
      break;
    case "account/usage/read":
      result = { summary: {}, dailyUsageBuckets: [] };
      break;
    case "account/rateLimits/read":
      result = { rateLimits: null };
      break;
  }
  send({ id: message.id, result });
});
