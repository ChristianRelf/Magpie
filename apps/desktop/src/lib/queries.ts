import { keepPreviousData, useQuery } from "@tanstack/react-query";
import type { ExecutionQuery, GroupBy, RangeQuery } from "@magpie/sdk";
import { useHarness } from "./harness";

/** Ready-gated query helper: only runs once the harness is connected. */
function useReady() {
  const { client, phase } = useHarness();
  return { client: client!, enabled: phase === "ready" && !!client };
}

export function useStatus() {
  const interval = (useSettings().data?.settings.analytics.refresh_interval_secs ?? 5) * 1000;
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["status"],
    queryFn: () => client.status(),
    enabled,
    refetchInterval: interval,
  });
}

export function useProviders() {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["providers"],
    queryFn: () => client.providers(),
    enabled,
  });
}

export function useModels() {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["models"],
    queryFn: () => client.models(),
    enabled,
  });
}

export function useLimits() {
  const { client, enabled } = useReady();
  // Countdowns are computed client-side; refetch occasionally to pick up resets.
  return useQuery({
    queryKey: ["limits"],
    queryFn: () => client.limits(),
    enabled,
    refetchInterval: 60_000,
  });
}

export function useUsageSummary(q: RangeQuery) {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["usage", "summary", q],
    queryFn: () => client.usageSummary(q),
    enabled,
    placeholderData: keepPreviousData,
  });
}

export function useTimeseries(q: RangeQuery & { group_by?: GroupBy; bucket_ms?: number }, active = true) {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["usage", "timeseries", q],
    queryFn: () => client.usageTimeseries(q),
    enabled: enabled && active,
    placeholderData: keepPreviousData,
  });
}

export function useBreakdown(q: RangeQuery & { group_by?: GroupBy }) {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["usage", "breakdown", q],
    queryFn: () => client.usageBreakdown(q),
    enabled,
    placeholderData: keepPreviousData,
  });
}

export function useProviderReports() {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["reports"],
    queryFn: () => client.providerReports(),
    enabled,
    refetchInterval: 30_000,
  });
}

export function useExecutions(q: ExecutionQuery) {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["executions", q],
    queryFn: () => client.executions(q),
    enabled,
    placeholderData: keepPreviousData,
  });
}

export function useExecution(id: string | null) {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["executions", "one", id],
    queryFn: () => client.execution(id!),
    enabled: enabled && !!id,
  });
}

export function useActive() {
  const interval = (useSettings().data?.settings.analytics.refresh_interval_secs ?? 5) * 1000;
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["active"],
    queryFn: () => client.activeExecutions(),
    enabled,
    refetchInterval: interval,
  });
}

export function useSettings() {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["settings"],
    queryFn: () => client.settings(),
    enabled,
    // Settings events and reconnects invalidate this cache. Mounting another
    // tab should not repeat the OS start-on-login check.
    refetchOnMount: false,
  });
}

export function useRouting() {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["routing"],
    queryFn: () => client.routing(),
    enabled,
  });
}

export function useKeys() {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["keys"],
    queryFn: () => client.keys(),
    enabled,
  });
}

export function useNotifications() {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["notifications"],
    queryFn: () => client.notifications(),
    enabled,
  });
}

export function useClis(open = true) {
  const { client, enabled } = useReady();
  return useQuery({
    queryKey: ["clis"],
    queryFn: () => client.detectClis(),
    enabled: enabled && open,
    staleTime: 10_000,
  });
}
