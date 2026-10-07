import { useState, type ReactNode } from "react";
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { ArrowDown, ArrowUp, GripVertical, Plus, X } from "lucide-react";
import type {
  RoutingConfig,
  RoutingDecision,
  RoutingPreset,
  TaskClass,
} from "@magpie/sdk";
import { Page } from "@/components/Page";
import {
  Button,
  Field,
  IconButton,
  Input,
  Panel,
  PanelHeader,
  Textarea,
} from "@/components/ui/core";
import { Select, SettingRow, Switch } from "@/components/ui/controls";
import { QueryState } from "@/components/QueryState";
import { useLimits, useModels, useProviders, useRouting } from "@/lib/queries";
import { useClient } from "@/lib/harness";
import { useAction } from "@/lib/action";
import { providerName, TASK_LABELS, usd } from "@/lib/format";

export const ROUTING_PRESETS: {
  value: RoutingPreset;
  label: string;
  description: string;
}[] = [
  {
    value: "automatic",
    label: "Automatic",
    description: "Balance capability, quality, latency and cost.",
  },
  {
    value: "best_quality",
    label: "Best quality",
    description: "Prefer the strongest eligible models.",
  },
  {
    value: "fastest",
    label: "Fastest",
    description: "Prefer models with lower response latency.",
  },
  {
    value: "economical",
    label: "Economical",
    description: "Minimise estimated marginal cost.",
  },
  {
    value: "preserve_limits",
    label: "Preserve limits",
    description: "Spread work using known allowances.",
  },
  {
    value: "manual",
    label: "Manual",
    description: "Use the model you specify below.",
  },
];

function SortableRow({
  id,
  children,
  up,
  down,
  remove,
}: {
  id: string;
  children: ReactNode;
  up?: () => void;
  down?: () => void;
  remove?: () => void;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id });
  return (
    <div
      ref={setNodeRef}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
        opacity: isDragging ? 0.5 : 1,
      }}
      className="flex items-center gap-2 rounded-md border border-border bg-surface px-2 py-2"
    >
      <button
        {...attributes}
        {...listeners}
        aria-label={`Drag ${id}`}
        title="Drag to reorder"
        className="cursor-grab touch-none text-fg-subtle"
      >
        <GripVertical className="size-4" />
      </button>
      <span className="min-w-0 flex-1 truncate text-xs">{children}</span>
      <IconButton label="Move up" size="xs" disabled={!up} onClick={up}>
        <ArrowUp className="size-3" />
      </IconButton>
      <IconButton label="Move down" size="xs" disabled={!down} onClick={down}>
        <ArrowDown className="size-3" />
      </IconButton>
      {remove && (
        <IconButton
          label="Remove from fallback order"
          size="xs"
          onClick={remove}
        >
          <X className="size-3" />
        </IconButton>
      )}
    </div>
  );
}

function PriorityList({
  ids,
  change,
  label,
  removable = false,
}: {
  ids: string[];
  change: (ids: string[]) => void;
  label: (id: string) => string;
  removable?: boolean;
}) {
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );
  const end = ({ active, over }: DragEndEvent) => {
    if (over && active.id !== over.id)
      change(
        arrayMove(
          ids,
          ids.indexOf(String(active.id)),
          ids.indexOf(String(over.id)),
        ),
      );
  };
  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragEnd={end}
    >
      <SortableContext items={ids} strategy={verticalListSortingStrategy}>
        <div className="space-y-1.5">
          {ids.map((id, i) => (
            <SortableRow
              key={id}
              id={id}
              up={i ? () => change(arrayMove(ids, i, i - 1)) : undefined}
              down={
                i < ids.length - 1
                  ? () => change(arrayMove(ids, i, i + 1))
                  : undefined
              }
              remove={
                removable
                  ? () => change(ids.filter((x) => x !== id))
                  : undefined
              }
            >
              {label(id)}
            </SortableRow>
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
}

function RoutingEditor({ initial }: { initial: RoutingConfig }) {
  const [draft, setDraft] = useState(initial);
  const [saved, setSaved] = useState(initial);
  const models = useModels().data ?? [];
  const providers = useProviders().data?.accounts ?? [];
  const limits = useLimits().data?.accounts ?? [];
  const action = useAction();
  const client = useClient();
  const [preview, setPreview] = useState("");
  const [decision, setDecision] = useState<RoutingDecision | null>(null);
  const patch = (p: Partial<RoutingConfig>) => setDraft({ ...draft, ...p });
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const modelOptions = models.map((m) => ({
    value: m.key,
    label: `${m.display_name} · ${m.account_label}`,
  }));
  const providerOrder = [
    ...draft.provider_order,
    ...[...new Set(providers.map((p) => p.kind))].filter(
      (p) => !draft.provider_order.includes(p),
    ),
  ];
  const reliable = limits.some((a) =>
    a.windows.some(
      (w) =>
        w.provenance === "reported" &&
        (w.used_percent !== undefined ||
          (w.limit !== undefined && w.remaining !== undefined)),
    ),
  );
  return (
    <Page
      title="Routing"
      subtitle={dirty ? "Unsaved changes" : "Preferences saved locally"}
      actions={
        <>
          <Button disabled={!dirty} onClick={() => setDraft(saved)}>
            Discard
          </Button>
          <Button
            variant="primary"
            loading={action.busy}
            disabled={
              !dirty || (draft.preset === "manual" && !draft.manual_model)
            }
            onClick={() =>
              void action.run(
                async () => {
                  const result = await client.updateRouting(draft);
                  setDraft(result);
                  setSaved(result);
                },
                "Routing preferences saved",
                ["routing"],
              )
            }
          >
            Save changes
          </Button>
        </>
      }
    >
      <div className="mx-auto max-w-5xl space-y-5 p-5">
        <div
          className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3"
          role="radiogroup"
          aria-label="Routing preset"
        >
          {ROUTING_PRESETS.map((p) => (
            <button
              key={p.value}
              role="radio"
              aria-checked={draft.preset === p.value}
              onClick={() => patch({ preset: p.value })}
              className={`rounded-lg border p-3 text-left transition-colors ${draft.preset === p.value ? "border-fg-muted bg-surface-3" : "border-border bg-surface hover:border-border-strong"}`}
            >
              <div className="text-[13px] font-medium">{p.label}</div>
              <p className="mt-1 text-xs text-fg-subtle">{p.description}</p>
            </button>
          ))}
        </div>
        {draft.preset === "manual" && (
          <Field label="Manual model">
            <Select
              label="Manual model"
              value={draft.manual_model ?? undefined}
              placeholder="Choose a model"
              onChange={(manual_model) => patch({ manual_model })}
              options={modelOptions}
            />
          </Field>
        )}
        <div className="grid gap-4 lg:grid-cols-2">
          <Panel>
            <PanelHeader
              title="Provider priority"
              subtitle="Drag rows or use the arrow controls"
            />
            <div className="p-3">
              <PriorityList
                ids={providerOrder}
                label={providerName}
                change={(ids) =>
                  patch({
                    provider_order: ids as RoutingConfig["provider_order"],
                  })
                }
              />
              {!providerOrder.length && (
                <p className="p-3 text-xs text-fg-subtle">
                  Connect providers to set an order.
                </p>
              )}
            </div>
          </Panel>
          <Panel>
            <PanelHeader
              title="Fallback order"
              subtitle="Eligible models only; capability requirements still apply"
            />
            <div className="space-y-3 p-3">
              <PriorityList
                ids={draft.fallback_order}
                label={(id) =>
                  modelOptions.find((o) => o.value === id)?.label ?? id
                }
                removable
                change={(fallback_order) => patch({ fallback_order })}
              />
              <Select
                label="Add fallback model"
                value=""
                placeholder="Add a model…"
                options={modelOptions.filter(
                  (m) => !draft.fallback_order.includes(m.value),
                )}
                onChange={(id) =>
                  patch({ fallback_order: [...draft.fallback_order, id] })
                }
              />
            </div>
          </Panel>
        </div>
        <Panel>
          <PanelHeader title="Execution policy" />
          <div className="divide-y divide-border">
            <SettingRow
              title="Automatic fallback"
              description="Retry eligible alternatives before any output or tool effects are committed."
            >
              <Switch
                label="Automatic fallback"
                checked={draft.allow_fallback}
                onCheckedChange={(allow_fallback) => patch({ allow_fallback })}
              />
            </SettingRow>
            <SettingRow
              title="Allow metered APIs"
              description="Allow explicitly connected billable accounts in automatic selection."
            >
              <Switch
                label="Allow metered APIs"
                checked={draft.allow_metered}
                onCheckedChange={(allow_metered) => patch({ allow_metered })}
              />
            </SettingRow>
            <SettingRow
              title="Subscription to API fallback"
              description="Opt in to potential API charges when a subscription becomes unavailable."
            >
              <Switch
                label="Subscription to API fallback"
                checked={draft.allow_subscription_to_api}
                onCheckedChange={(allow_subscription_to_api) =>
                  patch({ allow_subscription_to_api })
                }
              />
            </SettingRow>
            <SettingRow
              title="Maximum estimated cost per request"
              description="USD. Models with unknown prices are excluded when a cap is set."
            >
              <Input
                aria-label="Maximum cost"
                className="w-32"
                type="number"
                min={0}
                step={0.01}
                placeholder="No cap"
                value={draft.max_cost_per_request_usd ?? ""}
                onChange={(e) =>
                  patch({
                    max_cost_per_request_usd: e.target.value
                      ? Math.max(0, Number(e.target.value))
                      : null,
                  })
                }
              />
            </SettingRow>
            <SettingRow
              title="Reserve premium allowance"
              description={
                reliable
                  ? "Percentage reserved for high-complexity tasks."
                  : "Unavailable: no reliable remaining allowance has been reported."
              }
            >
              <Input
                aria-label="Reserve premium percentage"
                disabled={!reliable}
                className="w-32"
                type="number"
                min={0}
                max={90}
                placeholder="Disabled"
                value={draft.preserve_premium_percent ?? ""}
                onChange={(e) =>
                  patch({
                    preserve_premium_percent: e.target.value
                      ? Math.max(0, Math.min(90, Number(e.target.value)))
                      : null,
                  })
                }
              />
            </SettingRow>
            <SettingRow
              title="Quality preference"
              description="Adjust the balance used by Automatic."
            >
              <Select
                label="Quality bias"
                className="w-40"
                value={String(draft.quality_bias)}
                onChange={(v) => patch({ quality_bias: Number(v) })}
                options={[
                  { value: "-1", label: "Lower cost" },
                  { value: "0", label: "Balanced" },
                  { value: "1", label: "Higher quality" },
                ]}
              />
            </SettingRow>
            <SettingRow
              title="Learn from local performance"
              description="Use observed response times and success rates when ranking models."
            >
              <Switch
                label="Learn from history"
                checked={draft.learn_from_history}
                onCheckedChange={(learn_from_history) =>
                  patch({ learn_from_history })
                }
              />
            </SettingRow>
          </div>
        </Panel>
        <Panel>
          <PanelHeader
            title="Task defaults"
            subtitle="Preferred models for a specific task classification"
            actions={
              <Button
                size="xs"
                icon={<Plus className="size-3" />}
                disabled={
                  draft.task_rules.length >= Object.keys(TASK_LABELS).length
                }
                onClick={() => {
                  const task = (Object.keys(TASK_LABELS) as TaskClass[]).find(
                    (t) => !draft.task_rules.some((r) => r.task === t),
                  );
                  if (task)
                    patch({
                      task_rules: [...draft.task_rules, { task, models: [] }],
                    });
                }}
              >
                Add rule
              </Button>
            }
          />
          <div className="space-y-2 p-3">
            {!draft.task_rules.length && (
              <p className="p-2 text-xs text-fg-subtle">
                All tasks use your default routing preset.
              </p>
            )}
            {draft.task_rules.map((rule, i) => (
              <div key={i} className="flex items-center gap-2">
                <Select
                  className="w-48"
                  label={`Task rule ${i + 1}`}
                  value={rule.task}
                  options={Object.entries(TASK_LABELS)
                    .filter(
                      ([k]) =>
                        k === rule.task ||
                        !draft.task_rules.some((r) => r.task === k),
                    )
                    .map(([value, label]) => ({ value, label }))}
                  onChange={(task) =>
                    patch({
                      task_rules: draft.task_rules.map((r, n) =>
                        n === i ? { ...r, task: task as TaskClass } : r,
                      ),
                    })
                  }
                />
                <Select
                  label={`Preferred model for ${rule.task}`}
                  className="flex-1"
                  value={rule.models[0] ?? "default"}
                  options={[
                    { value: "default", label: "Automatic model" },
                    ...modelOptions,
                  ]}
                  onChange={(id) =>
                    patch({
                      task_rules: draft.task_rules.map((r, n) =>
                        n === i
                          ? { ...r, models: id === "default" ? [] : [id] }
                          : r,
                      ),
                    })
                  }
                />
                <IconButton
                  label="Remove task rule"
                  onClick={() =>
                    patch({
                      task_rules: draft.task_rules.filter((_, n) => n !== i),
                    })
                  }
                >
                  <X className="size-3.5" />
                </IconButton>
              </div>
            ))}
          </div>
        </Panel>
        <Panel>
          <PanelHeader
            title="Inspect a routing decision"
            subtitle="Uses saved preferences. Classification is local; no provider execution or charge."
          />
          <div className="space-y-3 p-4">
            <Textarea
              aria-label="Sample task for routing preview"
              rows={2}
              value={preview}
              onChange={(e) => setPreview(e.target.value)}
              placeholder="Describe a task to see which models are eligible…"
            />
            <Button
              loading={action.busy}
              disabled={!preview.trim() || dirty}
              onClick={() =>
                void action.run(async () =>
                  setDecision(
                    await client.route({ model: "auto", input: preview }),
                  ),
                )
              }
            >
              Inspect route
            </Button>
            {decision && (
              <div className="space-y-2 text-xs">
                <p className="text-fg-subtle">
                  {TASK_LABELS[decision.classification.task]} ·{" "}
                  {decision.classification.complexity} complexity
                </p>
                {decision.candidates.map((c, i) => (
                  <details
                    key={c.model.key}
                    open={i === 0}
                    className="rounded-md border border-border p-3"
                  >
                    <summary className="cursor-pointer font-medium">
                      {i + 1}. {c.model.display_name}{" "}
                      <span className="ml-2 font-mono text-fg-subtle">
                        {usd(c.estimated_cost_usd)}
                      </span>
                    </summary>
                    <ul className="mt-2 space-y-1 text-fg-muted">
                      {c.reasons.map((r, n) => (
                        <li key={n}>{r}</li>
                      ))}
                    </ul>
                  </details>
                ))}
                {!decision.candidates.length && (
                  <p>
                    No eligible model. Connect a provider or adjust your
                    preferences.
                  </p>
                )}
              </div>
            )}
          </div>
        </Panel>
      </div>
    </Page>
  );
}

export function Routing() {
  const query = useRouting();
  return query.data ? (
    <RoutingEditor initial={query.data} />
  ) : (
    <Page title="Routing">
      <QueryState
        pending={query.isLoading}
        error={query.error}
        retry={query.refetch}
      />
    </Page>
  );
}
