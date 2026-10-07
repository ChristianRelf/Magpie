/** Optional real-provider smoke test. Never invoked by CI. */
if (!process.argv.includes("--allow-real-provider")) {
  throw Error(
    "This sends one request to your selected provider and may consume allowance or money. Pass --allow-real-provider to opt in.",
  );
}
const { MAGPIE_URL = "http://127.0.0.1:7878", MAGPIE_API_KEY, MAGPIE_SMOKE_MODEL } = process.env;
if (!MAGPIE_API_KEY || !MAGPIE_SMOKE_MODEL || MAGPIE_SMOKE_MODEL === "auto") {
  throw Error("Set MAGPIE_API_KEY and MAGPIE_SMOKE_MODEL to an explicit API/local model. Connect it in Magpie first.");
}
const url = new URL(MAGPIE_URL);
if (url.protocol !== "http:" || !["127.0.0.1", "localhost", "[::1]"].includes(url.hostname))
  throw Error("Use a loopback harness URL");
const r = await fetch(new URL("/v1/responses", url), {
  method: "POST",
  redirect: "error",
  signal: AbortSignal.timeout(60000),
  headers: { Authorization: `Bearer ${MAGPIE_API_KEY}`, "Content-Type": "application/json" },
  body: JSON.stringify({
    model: MAGPIE_SMOKE_MODEL,
    input: "Reply with OK.",
    max_output_tokens: 32,
    preferences: { allow_fallback: false, allow_billable: true },
  }),
});
const result = await r.json();
if (!r.ok) throw Error(result.error?.message ?? `HTTP ${r.status}`);
if (!result.output_text) throw Error("No text returned");
console.log(JSON.stringify({ id: result.id, model: result.model, usage: result.usage, cost: result.cost }));
