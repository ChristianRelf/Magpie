import { useMemo, useState } from "react";
import {
  Star,
  Search,
  Settings2,
  Boxes,
  Eye,
  Braces,
  Wrench,
  Brain,
  Workflow,
} from "lucide-react";
import type { ModelInfo, ModelPreference } from "@magpie/sdk";
import { Page } from "@/components/Page";
import {
  Badge,
  Button,
  EmptyState,
  Field,
  IconButton,
  Input,
  Mono,
  Panel,
  Tooltip,
} from "@/components/ui/core";
import { Dialog, Select, Switch } from "@/components/ui/controls";
import { ProvenanceTag } from "@/components/ui/feedback";
import { QueryState } from "@/components/QueryState";
import { useLimits, useModels } from "@/lib/queries";
import { useClient } from "@/lib/harness";
import { useAction } from "@/lib/action";
import { compact, providerName, titleCase, usd } from "@/lib/format";

function ModelEditor({
  model,
  onClose,
}: {
  model: ModelInfo;
  onClose: () => void;
}) {
  const [preference, setPreference] = useState<ModelPreference>(
    model.preference,
  );
  const client = useClient();
  const action = useAction();
  const limits = useLimits();
  const reliable = limits.data?.accounts
    .find((a) => a.account_id === model.account_id)
    ?.windows.some(
      (w) =>
        w.provenance === "reported" &&
        (!w.model_scope || w.model_scope === model.model_id) &&
        (w.used_percent !== undefined ||
          (w.limit !== undefined && w.remaining !== undefined)),
    );
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={model.display_name}
      description={`${model.account_label} · ${model.model_id}`}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            loading={action.busy}
            onClick={() =>
              void action.run(
                async () => {
                  await client.setModelPreference(model.key, preference);
                  onClose();
                },
                "Model preferences saved",
                ["models"],
              )
            }
          >
            Save preferences
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field
          label="Routing priority"
          hint="Higher values prefer this model within an eligible routing tier."
        >
          <Input
            type="number"
            min={-100}
            max={100}
            value={preference.priority}
            onChange={(e) =>
              setPreference({
                ...preference,
                priority: Math.max(-100, Math.min(100, Number(e.target.value))),
              })
            }
          />
        </Field>
        <Field
          label="Reserved allowance (%)"
          hint={
            reliable
              ? "Only reliable provider-reported allowances are used."
              : "Unavailable until the provider reports a reliable remaining allowance."
          }
        >
          <Input
            disabled={!reliable}
            type="number"
            min={0}
            max={95}
            value={preference.reserve_percent ?? ""}
            placeholder="Use global preference"
            onChange={(e) =>
              setPreference({
                ...preference,
                reserve_percent: e.target.value
                  ? Math.max(0, Math.min(95, Number(e.target.value)))
                  : undefined,
              })
            }
          />
        </Field>
        <div className="flex justify-between text-xs">
          <span>Favourite</span>
          <Switch
            label="Favourite model"
            checked={preference.favourite}
            onCheckedChange={(favourite) =>
              setPreference({ ...preference, favourite })
            }
          />
        </div>
        <div className="flex justify-between text-xs">
          <span>Enabled for routing</span>
          <Switch
            label="Model enabled"
            checked={!preference.disabled}
            onCheckedChange={(enabled) =>
              setPreference({ ...preference, disabled: !enabled })
            }
          />
        </div>
        <p className="text-xs text-fg-subtle">
          Context:{" "}
          {model.context_window ? compact(model.context_window) : "Unavailable"}{" "}
          · Quality: {titleCase(model.tier)} · Metadata:{" "}
          {titleCase(model.metadata_provenance)}
        </p>
        {model.description && (
          <p className="text-xs text-fg-subtle">{model.description}</p>
        )}
      </div>
    </Dialog>
  );
}

export function Models() {
  const models = useModels();
  const client = useClient();
  const action = useAction();
  const [search, setSearch] = useState("");
  const [provider, setProvider] = useState("all");
  const [filter, setFilter] = useState("all");
  const [selected, setSelected] = useState<ModelInfo | null>(null);
  const rows = useMemo(
    () =>
      (models.data ?? [])
        .filter(
          (m) =>
            `${m.display_name} ${m.model_id} ${m.account_label}`
              .toLowerCase()
              .includes(search.toLowerCase()) &&
            (provider === "all" || m.provider === provider) &&
            (filter === "all" ||
              (filter === "favourites"
                ? m.preference.favourite
                : m.available && !m.preference.disabled)),
        )
        .sort(
          (a, b) =>
            Number(b.preference.favourite) - Number(a.preference.favourite) ||
            b.preference.priority - a.preference.priority ||
            a.display_name.localeCompare(b.display_name),
        ),
    [models.data, search, provider, filter],
  );
  return (
    <Page
      title="Models"
      subtitle={`${rows.length} models`}
      toolbar={
        <>
          <div className="relative w-64">
            <Search className="absolute top-2.5 left-2.5 size-3 text-fg-subtle" />
            <Input
              aria-label="Search models"
              className="pl-8"
              placeholder="Search models…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          <Select
            label="Provider"
            className="w-44"
            value={provider}
            onChange={setProvider}
            options={[
              { value: "all", label: "All providers" },
              ...[...new Set((models.data ?? []).map((m) => m.provider))].map(
                (p) => ({ value: p, label: providerName(p) }),
              ),
            ]}
          />
          <Select
            label="Model filter"
            className="w-40"
            value={filter}
            onChange={setFilter}
            options={[
              { value: "all", label: "All models" },
              { value: "favourites", label: "Favourites" },
              { value: "available", label: "Available" },
            ]}
          />
        </>
      }
    >
      <QueryState
        pending={models.isLoading}
        error={models.error}
        retry={models.refetch}
      />
      {models.isSuccess && !rows.length ? (
        <EmptyState
          icon={<Boxes />}
          title="No matching models"
          description="Connect a provider to discover models, or adjust your filters."
        />
      ) : (
        <Panel className="m-5 overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="border-b border-border text-fg-subtle">
              <tr>
                {[
                  "",
                  "Model",
                  "Provider",
                  "Capabilities",
                  "Context",
                  "Speed",
                  "Cost / 1M tokens",
                  "Availability",
                  "",
                ].map((h, i) => (
                  <th key={i} scope="col" className="px-3 py-2.5 font-normal">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {rows.map((m) => (
                <tr key={m.key} className="hover:bg-surface-2">
                  <td className="pl-2">
                    <IconButton
                      label={
                        m.preference.favourite
                          ? "Remove favourite"
                          : "Favourite model"
                      }
                      disabled={action.busy}
                      onClick={() =>
                        void action.run(
                          () =>
                            client.setModelPreference(m.key, {
                              ...m.preference,
                              favourite: !m.preference.favourite,
                            }),
                          undefined,
                          ["models"],
                        )
                      }
                    >
                      <Star
                        className={`size-3.5 ${m.preference.favourite ? "fill-current text-fg" : ""}`}
                      />
                    </IconButton>
                  </td>
                  <td className="max-w-60 px-3 py-3">
                    <button
                      onClick={() => setSelected(m)}
                      className="block text-left"
                    >
                      <span className="block truncate font-medium">
                        {m.display_name}
                      </span>
                      <span className="mt-0.5 block truncate font-mono text-2xs text-fg-subtle">
                        {m.model_id}
                      </span>
                    </button>
                  </td>
                  <td className="px-3 py-3">
                    <div>{providerName(m.provider)}</div>
                    <div className="mt-0.5 text-2xs text-fg-subtle">
                      {m.account_label}
                    </div>
                  </td>
                  <td className="px-3">
                    <div className="flex gap-2">
                      {(
                        [
                          { key: "vision", icon: Eye, label: "Image inputs" },
                          { key: "tools", icon: Wrench, label: "Tool calling" },
                          {
                            key: "structured_output",
                            icon: Braces,
                            label: "Structured output",
                          },
                          { key: "reasoning", icon: Brain, label: "Reasoning" },
                          {
                            key: "agentic",
                            icon: Workflow,
                            label: "Agentic execution",
                          },
                        ] as const
                      )
                        .filter((c) => m.capabilities[c.key])
                        .map((c) => (
                          <Tooltip key={c.key} content={c.label}>
                            <span tabIndex={0} aria-label={c.label}>
                              <c.icon className="size-3.5 text-fg-muted" />
                            </span>
                          </Tooltip>
                        ))}
                    </div>
                  </td>
                  <td className="px-3">
                    <Mono>{compact(m.context_window)}</Mono>
                  </td>
                  <td className="px-3 text-fg-muted">{titleCase(m.speed)}</td>
                  <td className="px-3">
                    <div className="flex items-center gap-1.5">
                      <Mono>
                        {m.pricing
                          ? `${usd(m.pricing.input_per_mtok)} / ${usd(m.pricing.output_per_mtok)}`
                          : "Unavailable"}
                      </Mono>
                      {m.pricing && (
                        <ProvenanceTag provenance={m.pricing.provenance} />
                      )}
                    </div>
                  </td>
                  <td className="px-3">
                    <Tooltip content={m.unavailable_reason}>
                      <span>
                        <Badge>
                          {m.preference.disabled
                            ? "Disabled"
                            : m.available
                              ? "Available"
                              : "Unavailable"}
                        </Badge>
                      </span>
                    </Tooltip>
                  </td>
                  <td className="pr-2">
                    <IconButton
                      label={`Configure ${m.display_name}`}
                      onClick={() => setSelected(m)}
                    >
                      <Settings2 className="size-3.5" />
                    </IconButton>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Panel>
      )}
      {selected && (
        <ModelEditor model={selected} onClose={() => setSelected(null)} />
      )}
    </Page>
  );
}
