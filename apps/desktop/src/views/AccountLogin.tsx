import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ExternalLink, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/core";
import { errorMessage } from "@/components/ui/feedback";
import { useClient } from "@/lib/harness";
import { openExternal } from "@/lib/desktop";

/** Only the official Codex process handles the browser callback and tokens. */
export function AccountLogin({
  accountId,
  autoStart = false,
  onConnected,
}: {
  accountId: string;
  autoStart?: boolean;
  onConnected?: () => void;
}) {
  const client = useClient();
  const qc = useQueryClient();
  const [waiting, setWaiting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [url, setUrl] = useState<string>();
  const [error, setError] = useState<string>();
  const started = useRef(false);
  const completed = useRef(onConnected);
  completed.current = onConnected;

  const begin = async () => {
    setBusy(true);
    setError(undefined);
    try {
      const login = await client.loginProvider(accountId);
      setUrl(login.auth_url);
      setWaiting(true);
      await openExternal(login.auth_url);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (autoStart && !started.current) {
      started.current = true;
      void begin();
    }
  }, [autoStart]);

  useEffect(() => {
    if (!waiting) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const deadline = Date.now() + 5 * 60_000;
    const poll = async () => {
      try {
        const account = (await client.providers()).accounts.find((a) => a.id === accountId);
        if (cancelled) return;
        if (!account) {
          setWaiting(false);
          setError("This profile was disconnected.");
          return;
        }
        if (account.status === "connected") {
          setWaiting(false);
          for (const key of ["providers", "models", "status", "limits"]) void qc.invalidateQueries({ queryKey: [key] });
          completed.current?.();
          return;
        }
        if (account.status_message && !account.status_message.startsWith("Complete browser sign-in"))
          setError(account.status_message);
      } catch (e) {
        if (!cancelled) setError(errorMessage(e));
      }
      if (cancelled) return;
      if (Date.now() >= deadline) {
        setWaiting(false);
        setError("Sign-in has not completed. You can try again or return to this profile later.");
      } else timer = setTimeout(() => void poll(), 2500);
    };
    timer = setTimeout(() => void poll(), 1500);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [waiting, accountId, client, qc]);

  return (
    <div className="space-y-3 rounded-lg border border-border bg-bg-subtle p-3">
      <p className="text-xs leading-relaxed text-fg-muted">
        {waiting
          ? "Complete sign-in in your browser. Magpie will detect when this profile is ready."
          : "Sign in with ChatGPT to save this separate Codex profile."}{" "}
        Codex stores and refreshes its credentials in your operating system's credential manager.
      </p>
      <div className="flex flex-wrap gap-2">
        {waiting && url && (
          <Button
            size="sm"
            icon={<ExternalLink className="size-3.5" />}
            onClick={() => void openExternal(url).catch((e) => setError(errorMessage(e)))}
          >
            Open sign-in page
          </Button>
        )}
        <Button
          size="sm"
          loading={busy}
          icon={waiting ? <RefreshCw className="size-3.5" /> : <ExternalLink className="size-3.5" />}
          onClick={() => void begin()}
        >
          {waiting ? "Restart sign-in" : "Sign in with ChatGPT"}
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-xs text-fg">
          {error}
        </p>
      )}
    </div>
  );
}
