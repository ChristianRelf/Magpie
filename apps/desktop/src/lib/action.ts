import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { errorMessage, useToast } from "@/components/ui/feedback";

/** Await mutations and keep failures visible; never optimistically claim success. */
export function useAction() {
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const qc = useQueryClient();
  const run = async (
    action: () => Promise<unknown>,
    title?: string,
    invalidate: string[] = [],
  ) => {
    if (busy) return false;
    setBusy(true);
    try {
      await action();
      for (const key of invalidate)
        await qc.invalidateQueries({ queryKey: [key] });
      if (title) toast({ title, tone: "success" });
      return true;
    } catch (e) {
      toast({
        title: "Action failed",
        description: errorMessage(e),
        tone: "error",
      });
      return false;
    } finally {
      setBusy(false);
    }
  };
  return { busy, run };
}
