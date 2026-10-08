import { test, expect } from "@playwright/test";
import { copyFile, chmod, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";

test("saved profiles complete login without the dialog and remain independently manageable", async ({
  page,
  request,
}) => {
  test.skip(process.platform === "win32", "The scripted CLI fixture uses a Unix shebang");
  const failures: string[] = [];
  page.on("pageerror", (e) => failures.push(e.message));
  const { url, token, data } = JSON.parse(await readFile(".e2e-connection.json", "utf8"));
  const headers = { Authorization: `Bearer ${token}` };
  const cli = join(data, "profile-cli-fixture");
  await copyFile(resolve("e2e/fixtures/profile-cli.cjs"), cli);
  await chmod(cli, 0o700);
  const settings = (await (await request.get(`${url}/v1/settings`, { headers })).json()).settings;
  expect(
    (await request.put(`${url}/v1/settings`, { headers, data: { ...settings, onboarding_complete: true } })).ok(),
  ).toBeTruthy();
  // No browser consent or real credentials are used by this fixture.
  await page.addInitScript(() => {
    window.open = () => null;
  });
  await page.route("**/v1/providers/cli", (route) =>
    route.fulfill({
      json: {
        clis: ["codex_cli", "claude_code"].map((kind) => ({
          kind,
          binary: kind,
          installed: true,
          version: "test fixture",
        })),
      },
    }),
  );
  await page.route("**/v1/providers", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    const body = route.request().postDataJSON();
    return route.continue({ postData: JSON.stringify({ ...body, options: { cli_path: cli } }) });
  });
  const accounts = async () => (await (await request.get(`${url}/v1/providers`, { headers })).json()).accounts;
  await page.goto("/");
  await page.getByRole("button", { name: "Providers", exact: true }).click();
  for (const label of ["Work sign-in fixture", "Personal sign-in fixture"]) {
    await page.getByRole("button", { name: "Connect provider", exact: true }).click();
    await page.getByRole("button", { name: /^Codex CLI detected/ }).click();
    await expect(page.getByRole("combobox", { name: "Authentication", exact: true })).toContainText(
      "Separate browser sign-in",
    );
    await page.getByLabel("Name", { exact: true }).fill(label);
    await page.getByRole("button", { name: "Create sign-in profile", exact: true }).click();
    await expect(page.getByText("Complete sign-in in your browser.", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Finish later", exact: true }).click();
    await expect
      .poll(async () => (await accounts()).find((a: { label: string }) => a.label === label)?.status)
      .toBe("connected");
    await expect(page.getByText(label, { exact: true }).first()).toBeVisible();
  }
  const saved = await accounts();
  const work = saved.find((a: { label: string }) => a.label === "Work sign-in fixture");
  const personal = saved.find((a: { label: string }) => a.label === "Personal sign-in fixture");
  expect(work.id).not.toBe(personal.id);
  expect(work.identity).not.toBe(personal.identity);
  // Add another credential opens the correct provider's guided flow.
  const card = page
    .locator("div")
    .filter({ has: page.getByText("Work sign-in fixture", { exact: true }) })
    .filter({ has: page.getByRole("button", { name: "Manage", exact: true }) })
    .last();
  await card.getByRole("button", { name: "Manage", exact: true }).click();
  await page.getByRole("button", { name: "Add another credential", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("Connect Codex");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("button", { name: "Connect provider", exact: true }).click();
  await page.getByRole("button", { name: /^Claude Code CLI detected/ }).click();
  await page.getByLabel("Name", { exact: true }).fill("Claude saved token fixture");
  const secret = "sk-ant-oat01-test-fixture-never-a-real-token";
  await page.getByLabel("Claude Code token", { exact: true }).fill(secret);
  await page.getByRole("button", { name: "Save credential", exact: true }).click();
  await expect(page.getByText("Claude saved token fixture", { exact: true }).first()).toBeVisible();
  const tokenAccount = (await accounts()).find((a: { label: string }) => a.label === "Claude saved token fixture");
  expect(tokenAccount.auth_method).toBe("cli_token");
  expect(JSON.stringify(await accounts())).not.toContain(secret);
  expect(await page.locator("body").innerText()).not.toContain(secret);
  await page.screenshot({ path: "test-results/saved-auth-profiles.png", fullPage: true });
  expect((await request.delete(`${url}/v1/providers/${work.id}`, { headers })).status()).toBe(204);
  expect((await accounts()).find((a: { id: string }) => a.id === personal.id)?.status).toBe("connected");
  expect((await request.delete(`${url}/v1/providers/${personal.id}`, { headers })).status()).toBe(204);
  expect((await request.delete(`${url}/v1/providers/${tokenAccount.id}`, { headers })).status()).toBe(204);
  expect(failures).toEqual([]);
});
