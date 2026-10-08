import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import type { LimitWindow, ProviderAccount } from "@magpie/sdk";

test("plans show reported allowances, preserve unknowns and navigate with a smooth sidebar", async ({
  page,
  request,
}) => {
  const failures: string[] = [];
  page.on("pageerror", (e) => failures.push(e.message));
  const { url, token } = JSON.parse(await readFile(".e2e-connection.json", "utf8"));
  const response = await request.get(`${url}/v1/settings`, { headers: { Authorization: `Bearer ${token}` } });
  const settingsResponse = await response.json();
  settingsResponse.settings.onboarding_complete = true;
  Object.assign(settingsResponse.settings.general, { sidebar_collapsed: false, reduced_motion: false, theme: "dark" });
  await page.route("**/v1/settings", async (route) => {
    if (route.request().method() === "PUT") settingsResponse.settings = route.request().postDataJSON();
    await route.fulfill({ json: settingsResponse });
  });
  const now = Date.now();
  const window = (account_id: string, key: string, label: string, percent: number, resetMs: number): LimitWindow => ({
    account_id,
    key,
    label,
    used_percent: percent,
    resets_at: new Date(now + resetMs).toISOString(),
    observed_at: new Date(now).toISOString(),
    metric: "usage_percent",
    provenance: "reported",
    exhausted: percent === 100,
  });
  const account = (id: string, kind: ProviderAccount["kind"], label: string, plan?: string): ProviderAccount => ({
    id,
    kind,
    label,
    plan,
    auth_method: "cli_delegated",
    billing_mode: "subscription",
    billing_reported: true,
    status: "connected",
    enabled: true,
    created_at: new Date(now).toISOString(),
    has_secret: false,
    options: {},
    model_count: 3,
    available_model_count: 3,
    limits: null,
    capabilities: { tools: true, vision: true, reasoning: true, structured_output: true, agentic: true },
    descriptor: {
      kind,
      name: label,
      vendor: label,
      auth_method: "cli_delegated",
      default_billing: "subscription",
      default_base_url: null,
      cli_binary: null,
      cli_install: null,
      cli_login: null,
      key_url: null,
      docs_url: "https://example.com",
      is_local: false,
      allow_multiple: true,
      summary: "Explicit browser-test fixture",
    },
  });
  const accounts = [
    account("claude-plan", "claude_code", "Claude", "Max"),
    account("codex-plan", "codex_cli", "Codex", "Plus"),
    { ...account("api-plan", "openai", "OpenAI API"), auth_method: "api_key", billing_mode: "metered" },
  ];
  const limits = [
    {
      account_id: "claude-plan",
      state: "approaching",
      windows: [
        window("claude-plan", "session", "5-hour session", 37, 2 * 3_600_000),
        window("claude-plan", "weekly", "Weekly allowance", 82, 4 * 86_400_000),
      ],
    },
    {
      account_id: "codex-plan",
      state: "reset_pending",
      windows: [
        window("codex-plan", "session", "5-hour session", 100, -60_000),
        window("codex-plan", "weekly", "Weekly allowance", 24, 2 * 86_400_000),
      ],
    },
    { account_id: "api-plan", state: "unknown", windows: [] },
  ];
  // Provider details and the polled limits endpoint return the same explicit fixture.
  const fixtureAccounts = accounts.map((a) => ({ ...a, limits: limits.find((l) => l.account_id === a.id) }));
  await page.route("**/v1/providers", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, json: { ...(await response.json()), accounts: fixtureAccounts } });
  });
  await page.route("**/v1/limits", (route) => route.fulfill({ json: { accounts: limits } }));
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await page.getByRole("button", { name: "Overview", exact: true }).click();
  const claude = page.getByRole("article", { name: "Claude plan", exact: true });
  await expect(claude.getByText("Max", { exact: true })).toBeVisible();
  await expect(claude.getByRole("meter", { name: "5-hour session" })).toHaveAttribute("aria-valuenow", "37");
  await expect(claude.getByRole("meter", { name: "Weekly allowance" })).toHaveAttribute("aria-valuenow", "82");
  const codex = page.getByRole("article", { name: "Codex plan", exact: true });
  await expect(codex.getByText("Awaiting update", { exact: true })).toBeVisible();
  await expect(codex.getByRole("meter", { name: "5-hour session" })).toHaveCount(0);
  const api = page.getByRole("article", { name: "OpenAI API plan", exact: true });
  await expect(api.getByText("Allowance not reported")).toBeVisible();
  await expect(api.getByRole("meter")).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Token activity" })).toHaveCount(0);
  await page.screenshot({ path: "test-results/plans-dark.png" });

  await claude.getByRole("button", { name: "Manage Claude", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Claude", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByRole("button", { name: "Overview", exact: true }).click();
  const frames = await page.evaluate(async () => {
    const sidebar = document.querySelector<HTMLElement>('nav[aria-label="Main"]')!;
    const label = sidebar.querySelector('[aria-label="Overview"] .sidebar-label')!;
    const icon = sidebar.querySelector('[aria-label="Overview"] svg')!;
    const samples: { width: number; iconX: number; labelMounted: boolean }[] = [];
    sidebar.querySelector<HTMLButtonElement>('[aria-label="Collapse sidebar"]')!.click();
    const start = performance.now();
    while (performance.now() - start < 500) {
      await new Promise(requestAnimationFrame);
      samples.push({
        width: sidebar.getBoundingClientRect().width,
        iconX: icon.getBoundingClientRect().x,
        labelMounted: label.isConnected,
      });
    }
    return samples;
  });
  expect(frames.some((f) => f.width > 73 && f.width < 255)).toBe(true);
  expect(frames.every((f) => f.labelMounted)).toBe(true);
  expect(Math.max(...frames.map((f) => f.iconX)) - Math.min(...frames.map((f) => f.iconX))).toBeLessThan(1);
  await expect(page.getByRole("button", { name: "Expand sidebar" })).toHaveAttribute("aria-expanded", "false");
  await page.screenshot({ path: "test-results/plans-collapsed.png" });
  await page.getByRole("button", { name: "Analytics", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Analytics", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Overview", exact: true }).click();
  await page.getByRole("button", { name: "Expand sidebar" }).click();
  await expect(page.getByRole("navigation", { name: "Main" })).toHaveCSS("width", "256px");

  settingsResponse.settings.general.theme = "light";
  await page.reload();
  await expect(page.locator("html")).not.toHaveClass(/dark/);
  await expect(claude).toBeVisible();
  await page.screenshot({ path: "test-results/plans-light.png" });
  await page.setViewportSize({ width: 900, height: 620 });
  await expect(claude).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
  expect(overflow).toBe(false);
  await page.screenshot({ path: "test-results/plans-small.png" });
  await page.emulateMedia({ reducedMotion: "reduce" });
  const duration = await page
    .getByRole("navigation", { name: "Main" })
    .evaluate((e) => parseFloat(getComputedStyle(e).transitionDuration));
  expect(duration).toBeLessThan(0.001);
  expect(failures).toEqual([]);
});
