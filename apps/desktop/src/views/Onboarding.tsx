import { useState } from "react";
import { ArrowRight, Check, Plug, ShieldCheck } from "lucide-react";
import type { RoutingPreset } from "@magpie/sdk";
import { MagpieMark, ProviderMark } from "@/components/ui/marks";
import { Button } from "@/components/ui/core";
import { Switch } from "@/components/ui/controls";
import { useProviders, useRouting, useSettings } from "@/lib/queries";
import { useClient } from "@/lib/harness";
import { useAction } from "@/lib/action";
import { ConnectDialog } from "./ConnectDialog";
import { ROUTING_PRESETS } from "./Routing";

export function Onboarding() {
  const [connect, setConnect] = useState(false);
  const [preset, setPreset] = useState<RoutingPreset>("automatic");
  const [background, setBackground] = useState(false);
  const accounts = useProviders().data?.accounts ?? [];
  const settings = useSettings();
  const routing = useRouting();
  const client = useClient();
  const action = useAction();
  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto flex min-h-full max-w-2xl flex-col justify-center px-8 py-12">
        <div className="mb-8 flex items-center gap-3">
          <MagpieMark size={32} />
          <span className="text-lg font-semibold tracking-tight">Magpie</span>
          <span className="ml-auto font-mono text-2xs text-fg-subtle">
            LOCAL AI HARNESS
          </span>
        </div>
        <h1 className="text-3xl font-medium tracking-tight">
          One harness.
          <br />
          <span className="text-fg-subtle">Your models, working together.</span>
        </h1>
        <p className="mt-4 max-w-lg text-sm leading-relaxed text-fg-muted">
          Connect an AI provider and give your development tools a single,
          intelligent execution endpoint. Everything runs on this machine.
        </p>
        <div className="mt-8 space-y-6">
          <section>
            <div className="mb-3 flex items-center gap-2 text-sm font-medium">
              <span className="flex size-5 items-center justify-center rounded-full border border-border-strong text-2xs">
                1
              </span>
              Connect a provider
            </div>
            {accounts.map((a) => (
              <div
                key={a.id}
                className="mb-2 flex items-center gap-3 rounded-lg border border-border bg-surface p-3"
              >
                <ProviderMark kind={a.kind} size={24} />
                <span className="flex-1 text-sm">{a.label}</span>
                {a.status === "connected" ? (
                  <Check className="size-4" />
                ) : (
                  <span className="text-xs">{a.status}</span>
                )}
              </div>
            ))}
            <Button
              icon={<Plug className="size-3.5" />}
              variant={accounts.length ? "outline" : "primary"}
              onClick={() => setConnect(true)}
            >
              {accounts.length
                ? "Connect another provider"
                : "Connect provider"}
            </Button>
          </section>
          <section>
            <div className="mb-3 flex items-center gap-2 text-sm font-medium">
              <span className="flex size-5 items-center justify-center rounded-full border border-border-strong text-2xs">
                2
              </span>
              Choose how requests are routed
            </div>
            <div
              className="grid gap-2 sm:grid-cols-2"
              role="radiogroup"
              aria-label="Initial routing preference"
            >
              {ROUTING_PRESETS.filter((p) => p.value !== "manual").map((p) => (
                <button
                  key={p.value}
                  role="radio"
                  aria-checked={preset === p.value}
                  onClick={() => setPreset(p.value)}
                  className={`rounded-lg border p-3 text-left ${preset === p.value ? "border-fg-muted bg-surface-3" : "border-border bg-surface"}`}
                >
                  <span className="text-xs font-medium">{p.label}</span>
                  <p className="mt-1 text-2xs text-fg-subtle">
                    {p.description}
                  </p>
                </button>
              ))}
            </div>
          </section>
          <div className="flex items-center gap-4 rounded-lg border border-border p-3">
            <div className="flex-1">
              <p className="text-xs font-medium">
                Keep the harness running when I quit
              </p>
              <p className="mt-1 text-2xs text-fg-subtle">
                Optional. External tools can continue using Magpie in the
                background.
              </p>
            </div>
            <Switch
              label="Enable background harness"
              checked={background}
              onCheckedChange={setBackground}
            />
          </div>
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-1.5 text-2xs text-fg-subtle">
              <ShieldCheck className="size-3.5" />
              No hosted account. No prompt history by default.
            </span>
            <Button
              variant="primary"
              size="md"
              loading={action.busy}
              disabled={!settings.data || !routing.data}
              icon={<ArrowRight className="size-3.5" />}
              onClick={() =>
                void action.run(
                  async () => {
                    await client.updateRouting({ ...routing.data!, preset });
                    await client.updateSettings({
                      ...settings.data!.settings,
                      onboarding_complete: true,
                      general: {
                        ...settings.data!.settings.general,
                        keep_harness_running: background,
                      },
                    });
                  },
                  undefined,
                  ["routing", "settings"],
                )
              }
            >
              {accounts.length ? "Open Magpie" : "Explore first"}
            </Button>
          </div>
        </div>
        <ConnectDialog open={connect} onOpenChange={setConnect} />
      </div>
    </div>
  );
}
