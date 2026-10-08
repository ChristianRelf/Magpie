import { useEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Activity as ActivityIcon,
  ArrowLeft,
  ArrowRight,
  Square,
} from "lucide-react";
import { Page } from "@/components/Page";
import {
  Badge,
  Button,
  EmptyState,
  Mono,
  Panel,
  PanelHeader,
} from "@/components/ui/core";
import { Drawer, Select } from "@/components/ui/controls";
import { ExecutionGlyph, ProvenanceTag } from "@/components/ui/feedback";
import { QueryState } from "@/components/QueryState";
import {
  useActive,
  useExecution,
  useExecutions,
  useProviders,
} from "@/lib/queries";
import { useClient } from "@/lib/harness";
import { useNav } from "@/lib/nav";
import { useAction } from "@/lib/action";
import {
  compact,
  dateTime,
  ms,
  providerName,
  TASK_LABELS,
  titleCase,
  usd,
} from "@/lib/format";

function Inspector({ id, close }: { id: string; close: () => void }) {
  const query = useExecution(id);
  const providers = useProviders();
  const accountName = (id?: string) =>
    providers.data?.accounts.find((a) => a.id === id)?.label ?? id ?? "Unavailable";
  const e = query.data;
  const client = useClient();
  const action = useAction();
  return (
    <Drawer
      open
      onOpenChange={(o) => !o && close()}
      title="Execution inspector"
      actions={
        e?.status === "running" && (
          <Button
            loading={action.busy}
            icon={<Square className="size-3" />}
            onClick={() =>
              void action.run(
                () => client.cancel(id),
                "Cancellation requested",
                ["executions", "active"],
              )
            }
          >
            Cancel
          </Button>
        )
      }
    >
      <QueryState
        pending={query.isLoading}
        error={query.error}
        retry={query.refetch}
      />
      {e && (
        <div className="space-y-5 p-5">
          <div className="flex items-center gap-2">
            <ExecutionGlyph status={e.status} />
            <span className="font-medium">{titleCase(e.status)}</span>
            <Badge>{TASK_LABELS[e.task]}</Badge>
          </div>
          <code className="selectable block break-all text-xs text-fg-subtle">
            {e.id}
          </code>
          <dl className="grid grid-cols-2 gap-3 text-xs">
            {[
              ["Started", dateTime(e.created_at)],
              ["Model", e.model?.display_name ?? "No model selected"],
              ["Provider", providerName(e.model?.provider)],
              ["Connection", accountName(e.model?.account_id)],
              ["Client", e.client],
              ["Duration", ms(e.duration_ms)],
              ["First token", ms(e.time_to_first_token_ms)],
              ["Input tokens", compact(e.usage.input_tokens)],
              ["Output tokens", compact(e.usage.output_tokens)],
              ["Cached tokens", compact(e.usage.cached_input_tokens)],
              ["Reasoning tokens", compact(e.usage.reasoning_tokens)],
              [
                "Cost",
                e.cost
                  ? `${usd(e.cost.usd, { precise: true })}${e.cost.api_equivalent ? " equivalent" : ""} · ${e.cost.provenance}`
                  : "Unavailable",
              ],
              [
                "Finish reason",
                e.finish_reason ? titleCase(e.finish_reason) : "—",
              ],
            ].map(([label, value]) => (
              <div key={label}>
                <dt className="text-fg-subtle">{label}</dt>
                <dd className="selectable mt-1 break-words">{value}</dd>
              </div>
            ))}
          </dl>
          <div className="flex gap-2 text-xs text-fg-muted">
            <ProvenanceTag provenance={e.usage.provenance} />
            Token accounting: {e.usage.provenance}
          </div>
          {e.error && (
            <Panel className="p-3">
              <p className="text-xs font-medium">{titleCase(e.error.kind)}</p>
              <p className="selectable mt-1 text-xs text-fg-muted">
                {e.error.message}
              </p>
            </Panel>
          )}
          <Panel>
            <PanelHeader
              title="Routing decision"
              subtitle={titleCase(e.preset)}
            />
            <div className="space-y-3 p-3">
              {e.routing ? (
                <>
                  <p className="text-xs text-fg-muted">
                    {titleCase(e.routing.classification.complexity)} complexity
                    · {compact(e.routing.classification.estimated_input_tokens)}{" "}
                    estimated input tokens
                  </p>
                  {e.routing.candidates.map((c, i) => (
                    <details
                      key={c.model.key}
                      open={i === 0}
                      className="text-xs"
                    >
                      <summary className="cursor-pointer py-1.5">
                        {i + 1}. {c.model.display_name}
                        <span className="ml-2 text-fg-subtle">{accountName(c.model.account_id)}</span>
                        <span className="ml-2 font-mono text-fg-subtle">
                          {c.score.toFixed(2)}
                        </span>
                      </summary>
                      <ul className="space-y-1 pl-4 text-fg-muted">
                        {c.reasons.map((reason, j) => (
                          <li key={j}>{reason}</li>
                        ))}
                      </ul>
                    </details>
                  ))}
                  {e.routing.rejected.length > 0 && (
                    <details className="text-xs">
                      <summary className="cursor-pointer text-fg-subtle">
                        {e.routing.rejected.length} excluded models
                      </summary>
                      {e.routing.rejected.map((r) => (
                        <p key={r.model_key} className="py-1 text-fg-muted">
                          {r.display_name}: {r.reason}
                        </p>
                      ))}
                    </details>
                  )}
                </>
              ) : (
                <p className="text-xs text-fg-subtle">
                  Routing details unavailable.
                </p>
              )}
            </div>
          </Panel>
          <Panel>
            <PanelHeader title={`Attempts · ${e.attempts.length}`} />
            <div className="divide-y divide-border">
              {e.attempts.map((attempt, i) => (
                <div key={i} className="p-3 text-xs">
                  <div className="flex justify-between">
                    <span>{attempt.model.display_name}</span>
                    <Mono>{ms(attempt.duration_ms)}</Mono>
                  </div>
                  <p className="mt-1 text-fg-subtle">{accountName(attempt.model.account_id)}</p>
                  {attempt.error && (
                    <p className="mt-1 text-fg-subtle">
                      {attempt.error.message}
                    </p>
                  )}
                </div>
              ))}
            </div>
          </Panel>
          {e.request_content || e.response_content ? (
            <details className="text-xs">
              <summary className="cursor-pointer">Retained content</summary>
              <pre className="mt-3 whitespace-pre-wrap break-all rounded-md bg-bg p-3">
                {JSON.stringify(e.request_content, null, 2)}
                {e.response_content && `\n\n${e.response_content}`}
              </pre>
            </details>
          ) : (
            <p className="text-xs text-fg-subtle">
              Request and response content are not retained by default.
            </p>
          )}
        </div>
      )}
    </Drawer>
  );
}

export function Activity() {
  const [status, setStatus] = useState("all");
  const [provider, setProvider] = useState("all");
  const [task, setTask] = useState("all");
  const [offset, setOffset] = useState(0);
  const { param, navigate } = useNav();
  const [selected, setSelected] = useState<string | null>(param);
  useEffect(() => {
    if (param) setSelected(param);
  }, [param]);
  const query = useExecutions({
    limit: 100,
    offset,
    status: status === "all" ? undefined : status,
    provider: provider === "all" ? undefined : provider,
    task: task === "all" ? undefined : task,
  });
  const providers = useProviders();
  const active = useActive();
  const rows = query.data?.data ?? [];
  const scroll = useRef<HTMLDivElement>(null);
  const virtual = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroll.current,
    estimateSize: () => 48,
    overscan: 8,
  });
  const filter = (set: (s: string) => void) => (s: string) => {
    set(s);
    setOffset(0);
  };
  return (
    <Page
      title="Activity"
      subtitle={`${active.data?.data.length ?? 0} active · ${query.data?.total ?? 0} recorded`}
      scroll={false}
      toolbar={
        <>
          <Select
            className="w-36"
            label="Execution status"
            value={status}
            onChange={filter(setStatus)}
            options={["all", "running", "succeeded", "failed", "cancelled"].map(
              (s) => ({
                value: s,
                label: s === "all" ? "All statuses" : titleCase(s),
              }),
            )}
          />
          <Select
            className="w-44"
            label="Activity provider"
            value={provider}
            onChange={filter(setProvider)}
            options={[
              { value: "all", label: "All providers" },
              ...[
                ...new Set(providers.data?.accounts.map((a) => a.kind) ?? []),
              ].map((p) => ({ value: p, label: providerName(p) })),
            ]}
          />
          <Select
            className="w-44"
            label="Task classification"
            value={task}
            onChange={filter(setTask)}
            options={[
              { value: "all", label: "All task types" },
              ...Object.entries(TASK_LABELS).map(([value, label]) => ({
                value,
                label,
              })),
            ]}
          />
        </>
      }
    >
      <QueryState
        pending={query.isLoading}
        error={query.error}
        retry={query.refetch}
      />
      {query.isSuccess && !rows.length && (
        <EmptyState
          icon={<ActivityIcon />}
          title="No executions in this view"
          description="Requests from your connected tools appear here, with routing decisions, token usage, and timing."
        />
      )}
      {!!rows.length && (
        <>
          <div className="mx-5 mt-4 grid grid-cols-[145px_minmax(140px,1.5fr)_minmax(110px,1fr)_100px_80px_100px] border-b border-border px-3 py-2 text-2xs text-fg-subtle">
            <span>Time</span>
            <span>Model / provider</span>
            <span>Task</span>
            <span>Tokens</span>
            <span>Duration</span>
            <span>Status</span>
          </div>
          <div
            ref={scroll}
            className="mx-5 min-h-0 flex-1 overflow-auto"
            role="list"
            aria-label="Executions"
          >
            <div
              style={{
                height: virtual.getTotalSize(),
                position: "relative",
                minWidth: 760,
              }}
            >
              {virtual.getVirtualItems().map((item) => {
                const e = rows[item.index];
                return (
                  <button
                    key={e.id}
                    role="listitem"
                    onClick={() => setSelected(e.id)}
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      height: item.size,
                      transform: `translateY(${item.start}px)`,
                    }}
                    className="grid grid-cols-[145px_minmax(140px,1.5fr)_minmax(110px,1fr)_100px_80px_100px] items-center border-b border-border px-3 text-left text-xs hover:bg-surface-2"
                  >
                    <span className="text-2xs text-fg-subtle">
                      {dateTime(e.created_at)}
                    </span>
                    <span className="truncate">
                      <span className="block truncate">
                        {e.model?.display_name ?? "Unassigned"}
                      </span>
                      <span className="text-2xs text-fg-subtle">
                        {providerName(e.model?.provider)}
                      </span>
                    </span>
                    <span className="truncate text-fg-muted">
                      {TASK_LABELS[e.task]}
                    </span>
                    <Mono>
                      {e.usage.input_tokens !== undefined ||
                      e.usage.output_tokens !== undefined
                        ? compact(
                            (e.usage.input_tokens ?? 0) +
                              (e.usage.output_tokens ?? 0),
                          )
                        : "—"}
                    </Mono>
                    <Mono>{ms(e.duration_ms)}</Mono>
                    <span className="flex items-center gap-1.5">
                      <ExecutionGlyph status={e.status} />
                      {titleCase(e.status)}
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
          <div className="flex items-center justify-between border-t border-border px-5 py-3">
            <span className="text-xs text-fg-subtle">
              {offset + 1}–{offset + rows.length} of {query.data?.total}
            </span>
            <div className="flex gap-2">
              <Button
                disabled={offset === 0}
                icon={<ArrowLeft className="size-3" />}
                onClick={() => setOffset(Math.max(0, offset - 100))}
              >
                Previous
              </Button>
              <Button
                disabled={offset + 100 >= (query.data?.total ?? 0)}
                icon={<ArrowRight className="size-3" />}
                onClick={() => setOffset(offset + 100)}
              >
                Next
              </Button>
            </div>
          </div>
        </>
      )}
      {selected && (
        <Inspector
          id={selected}
          close={() => {
            setSelected(null);
            navigate("activity");
          }}
        />
      )}
    </Page>
  );
}
