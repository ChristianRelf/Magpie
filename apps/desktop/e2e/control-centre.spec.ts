import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";

test("first run, real local provider, execution, telemetry, settings and key revocation", async ({ page, request }) => {
  const failures: string[] = [];
  page.on("pageerror", (e) => failures.push(e.message));
  let settingsReads = 0;
  page.on("request", (r) => {
    if (new URL(r.url()).pathname === "/v1/settings" && r.method() === "GET") settingsReads++;
  });
  const { url, token } = JSON.parse(await readFile(".e2e-connection.json", "utf8"));
  const headers = { Authorization: `Bearer ${token}` };
  await page.goto("/");
  await expect(page.getByRole("heading", { name: /One harness/ })).toBeVisible();
  await page.getByRole("button", { name: "Connect provider", exact: true }).click();
  // Use real descriptors so a wire-name mismatch cannot silently hide a provider.
  const { kinds } = await (await request.get(`${url}/v1/providers`, { headers })).json();
  const picker = page.getByRole("dialog", { name: "Connect a provider", exact: true });
  for (const provider of kinds) {
    await expect(picker.getByText(provider.name, { exact: true })).toBeVisible();
  }
  await picker.getByRole("button", { name: /^OpenAI API/ }).click();
  await expect(page.getByRole("dialog", { name: /Connect OpenAI API$/ })).toBeVisible();
  await page.getByRole("button", { name: "Back", exact: true }).click();
  await page.getByRole("button", { name: /OpenAI-compatible/ }).click();
  await page.getByLabel("Base URL", { exact: true }).fill("http://127.0.0.1:17879/v1");
  await page.getByRole("combobox", { name: "Billing" }).click();
  await page.getByRole("option", { name: "Free / self-hosted" }).click();
  await page.getByLabel("Name", { exact: true }).fill("Isolated test provider");
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(page.getByText("Isolated test provider", { exact: true }).first()).toBeVisible();
  await page.getByRole("button", { name: "Open Magpie" }).click();
  await expect(page.getByRole("heading", { name: "Overview", exact: true })).toBeVisible();
  const response = await request.post(`${url}/v1/responses`, {
    headers,
    data: {
      model: "auto",
      input: "Test fixture request",
      max_output_tokens: 50,
    },
  });
  expect(response.status(), await response.text()).toBe(200);
  const result = await response.json();
  expect(result.output_text).toBe("Test fixture response.");
  expect(result.usage.input_tokens).toBe(12);
  // Let the settings cache become stale: changing tabs must not repeat OS probes.
  await page.waitForTimeout(5_100);
  const readsBeforeNavigation = settingsReads;
  for (const name of [
    "Providers",
    "Models",
    "Activity",
    "Analytics",
    "Routing",
    "Integrations",
    "Settings",
    "Overview",
  ]) {
    await page.getByRole("button", { name, exact: true }).click();
    await expect(page.getByRole("heading", { name, exact: true })).toBeVisible();
  }
  expect(settingsReads).toBe(readsBeforeNavigation);
  await expect(page.getByRole("heading", { name: "Current plans", exact: true })).toBeVisible();
  await expect(page.getByRole("article", { name: "Isolated test provider plan" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Token activity", exact: true })).toHaveCount(0);
  await page.screenshot({ path: "test-results/overview.png", fullPage: true });
  const navigation = page.getByRole("navigation", { name: "Main" });
  await expect(navigation).toHaveCSS("width", "256px");
  await page.getByRole("button", { name: "Collapse sidebar", exact: true }).click();
  await expect(navigation).toHaveCSS("width", "72px");
  await page.reload();
  await expect(navigation).toHaveCSS("width", "72px");
  await page.getByRole("button", { name: "Expand sidebar", exact: true }).click();
  await expect(navigation).toHaveCSS("width", "256px");
  await page.getByRole("button", { name: "Analytics", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Token activity", exact: true })).toBeVisible();
  for (const name of ["Weekly", "Cumulative", "Daily"]) await page.getByRole("radio", { name, exact: true }).click();
  await page.getByRole("button", { name: "Models", exact: true }).click();
  await page.getByRole("button", { name: "Favourite model", exact: true }).click();
  await expect(page.getByRole("button", { name: "Remove favourite", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Activity", exact: true }).click();
  await page.getByRole("listitem").first().click();
  await expect(page.getByRole("heading", { name: "Execution inspector" })).toBeVisible();
  await expect(page.getByText("Request and response content are not retained by default.")).toBeVisible();
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByRole("button", { name: "Routing", exact: true }).click();
  await page.getByRole("radio", { name: /Economical/ }).click();
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Preferences saved locally")).toBeVisible();
  expect((await (await request.get(`${url}/v1/routing`, { headers })).json()).preset).toBe("economical");
  await page.getByRole("button", { name: "Integrations", exact: true }).click();
  await page.getByRole("button", { name: "Create access key" }).click();
  await page.getByLabel("Application name").fill("E2E scoped client");
  await page.getByRole("button", { name: "Create key", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Save your access key" })).toBeVisible();
  const clientKey = await page.locator('[role="dialog"] code').textContent();
  expect(
    (
      await request.get(`${url}/v1/models`, {
        headers: { Authorization: `Bearer ${clientKey}` },
      })
    ).status(),
  ).toBe(200);
  expect(
    (
      await request.get(`${url}/v1/keys`, {
        headers: { Authorization: `Bearer ${clientKey}` },
      })
    ).status(),
  ).toBe(403);
  await page.getByRole("button", { name: "I have saved the key" }).click();
  await page.getByRole("button", { name: "Revoke", exact: true }).click();
  await page.getByRole("button", { name: "Revoke key", exact: true }).click();
  await expect(page.getByText("Key revoked", { exact: true })).toBeVisible();
  expect(
    (
      await request.get(`${url}/v1/models`, {
        headers: { Authorization: `Bearer ${clientKey}` },
      })
    ).status(),
  ).toBe(401);
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("switch", { name: "Reduce motion", exact: true }).click();
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Settings saved", { exact: true })).toBeVisible();
  await page.reload();
  await expect(page.getByRole("switch", { name: "Reduce motion", exact: true })).toBeChecked();
  expect((await (await request.get(`${url}/v1/usage/summary`, { headers })).json()).current.input_tokens).toBe(12);
  expect(failures).toEqual([]);

  // Explicit account-report fixture: no live Codex account or inference.
  // It overlaps harness activity, so the two sources must never be summed.
  const today = new Date().toISOString().slice(0, 10);
  const yesterday = new Date(Date.now() - 86_400_000).toISOString().slice(0, 10);
  let reported = 1234;
  let observed = new Date().toISOString();
  await page.route("**/v1/providers", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.accounts.push({
      ...body.accounts[0],
      id: "codex-fixture",
      kind: "codex_cli",
      label: "Codex test fixture",
      enabled: true,
      status: "connected",
      billing_mode: "subscription",
    });
    await route.fulfill({ response, json: body });
  });
  await page.route("**/v1/usage/provider-reports", (route) =>
    route.fulfill({
      json: {
        reports: [
          {
            account_id: "codex-fixture",
            provenance: "reported",
            observed_at: observed,
            lifetime_tokens: 90000,
            peak_daily_tokens: 8000,
            daily: [
              { date: today, tokens: reported },
              { date: yesterday, tokens: 4321 },
            ],
          },
        ],
      },
    }),
  );
  await page.route("**/v1/providers/codex-fixture/refresh", (route) => {
    reported = 7777;
    observed = new Date(Date.now() + 1000).toISOString();
    return route.fulfill({ json: { account: {}, models: null } });
  });
  await page.reload();
  await page.getByRole("button", { name: "Analytics", exact: true }).click();
  await expect(page.getByRole("combobox", { name: "Usage source" })).toContainText("Codex test fixture account");
  await expect(page.getByRole("img", { name: /Token activity over the last year: 5,555 tokens/ })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Account token usage", exact: true })).toBeVisible();
  await page.getByText("Accessible activity data", { exact: true }).click();
  await expect(page.getByRole("button", { name: /1,234 tokens/ })).toBeVisible();
  await page.getByRole("button", { name: "Refresh usage", exact: true }).click();
  await expect(page.getByRole("img", { name: /Token activity over the last year: 12,098 tokens/ })).toBeVisible();
  await page.screenshot({ path: "test-results/account-activity.png", fullPage: true });
  await page.getByRole("button", { name: "Analytics", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Account token usage", exact: true })).toBeVisible();
  await page.getByRole("combobox", { name: "Usage source" }).click();
  await page.getByRole("option", { name: "Magpie requests", exact: true }).click();
  await expect(page.getByRole("img", { name: /Token activity over the last year: 17 tokens/ })).toBeVisible();
  await page.reload();
  await expect(page.getByRole("combobox", { name: "Usage source" })).toContainText("Magpie requests");
  expect(failures).toEqual([]);
});
