import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { MagpieClient, type HarnessEvent } from "@magpie/sdk";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import {
  connectHarness,
  notifyOs,
  restartHarness,
  stopHarness,
  type HarnessConnection,
} from "./desktop";

type Phase = "connecting" | "ready" | "offline";

interface HarnessContextValue {
  phase: Phase;
  client: MagpieClient | null;
  connection: HarnessConnection | null;
  error: string | null;
  /** Start (or reconnect to) the harness. */
  start: () => Promise<void>;
  stop: () => Promise<void>;
  restart: () => Promise<void>;
}

const HarnessContext = createContext<HarnessContextValue | null>(null);

/** Map harness events to the queries they invalidate. */
function invalidationsFor(ev: HarnessEvent): string[][] {
  switch (ev.type) {
    case "execution_started":
      return [["active"], ["status"]];
    case "execution_finished":
      return [["active"], ["executions"], ["usage"], ["status"]];
    case "account_updated":
    case "account_removed":
      return [["providers"], ["models"], ["status"], ["limits"]];
    case "models_updated":
      return [["models"], ["providers"], ["status"]];
    case "limits_updated":
      return [["limits"], ["providers"], ["reports"]];
    case "notification":
      return [["notifications"]];
    case "settings_updated":
      return [["settings"]];
    case "routing_updated":
      return [["routing"]];
    default:
      return [];
  }
}

/** Batches invalidations so bursts of events cause one refetch. */
function makeInvalidator(qc: QueryClient) {
  const pending = new Map<string, string[]>();
  let timer: ReturnType<typeof setTimeout> | null = null;
  return (keys: string[][]) => {
    for (const k of keys) pending.set(k.join("/"), k);
    if (timer) return;
    timer = setTimeout(() => {
      timer = null;
      const batch = [...pending.values()];
      pending.clear();
      for (const k of batch) qc.invalidateQueries({ queryKey: k });
    }, 400);
  };
}

export function HarnessProvider({ children }: { children: ReactNode }) {
  const qc = useQueryClient();
  const [phase, setPhase] = useState<Phase>("connecting");
  const [connection, setConnection] = useState<HarnessConnection | null>(null);
  const [error, setError] = useState<string | null>(null);
  const stoppedByUser = useRef(false);
  const abort = useRef<AbortController | null>(null);

  const client = useMemo(
    () =>
      connection
        ? new MagpieClient({
            baseUrl: connection.url,
            apiKey: connection.token,
          })
        : null,
    [connection],
  );

  const connect = useCallback(async (start: boolean) => {
    setPhase((p) => (p === "ready" ? p : "connecting"));
    try {
      const c = await connectHarness(start);
      setConnection(c);
      setError(null);
      setPhase("ready");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPhase("offline");
    }
  }, []);

  useEffect(() => {
    void connect(true);
  }, [connect]);

  // Live events. When the stream drops, the harness is considered offline.
  useEffect(() => {
    if (!client || phase !== "ready") return;
    const ctrl = new AbortController();
    abort.current = ctrl;
    const invalidate = makeInvalidator(qc);
    (async () => {
      try {
        for await (const ev of client.events(ctrl.signal)) {
          invalidate(invalidationsFor(ev));
          if (ev.type === "notification")
            void notifyOs(ev.notification.title, ev.notification.body);
        }
      } catch {
        /* stream ended */
      }
      if (ctrl.signal.aborted) return;
      setPhase("offline");
      setError("The harness stopped responding.");
    })();
    return () => ctrl.abort();
  }, [client, phase, qc]);

  // While offline (and not stopped on purpose), poll health with backoff.
  useEffect(() => {
    if (!client || phase !== "offline" || stoppedByUser.current) return;
    let cancelled = false;
    (async () => {
      let delay = 1000;
      while (!cancelled) {
        await new Promise((r) => setTimeout(r, delay));
        if (cancelled) return;
        try {
          await client.health();
          setPhase("ready");
          setError(null);
          qc.invalidateQueries();
          return;
        } catch {
          delay = Math.min(delay * 2, 15000);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, phase, qc]);

  const value = useMemo<HarnessContextValue>(
    () => ({
      phase,
      client,
      connection,
      error,
      start: async () => {
        stoppedByUser.current = false;
        await connect(true);
        qc.invalidateQueries();
      },
      stop: async () => {
        stoppedByUser.current = true;
        abort.current?.abort();
        if (connection && !("__TAURI_INTERNALS__" in window))
          await client!.shutdown();
        else await stopHarness();
        setPhase("offline");
        setError("The harness is stopped.");
      },
      restart: async () => {
        stoppedByUser.current = false;
        abort.current?.abort();
        setPhase("connecting");
        try {
          const c = await restartHarness();
          setConnection(c);
          setPhase("ready");
          setError(null);
          qc.invalidateQueries();
        } catch (e) {
          setError(e instanceof Error ? e.message : String(e));
          setPhase("offline");
        }
      },
    }),
    [phase, client, connection, error, connect, qc],
  );

  return (
    <HarnessContext.Provider value={value}>{children}</HarnessContext.Provider>
  );
}

export function useHarness(): HarnessContextValue {
  const ctx = useContext(HarnessContext);
  if (!ctx) throw new Error("useHarness outside provider");
  return ctx;
}

/** The connected client; components using this render only when ready. */
export function useClient(): MagpieClient {
  const { client } = useHarness();
  if (!client) throw new Error("Harness not connected");
  return client;
}
