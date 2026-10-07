import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { RoutingConfig, Settings } from "@magpie/sdk";
import { useHarness } from "./harness";
import { useToast, errorMessage } from "@/components/ui/feedback";

/** Persist settings with an optimistic update. */
export function useSaveSettings() {
  const { client } = useHarness();
  const qc = useQueryClient();
  const toast = useToast();
  return useMutation({
    mutationFn: (s: Settings) => client!.updateSettings(s),
    onMutate: async (s) => {
      await qc.cancelQueries({ queryKey: ["settings"] });
      const prev = qc.getQueryData<{
        settings: Settings;
        autostart_enabled: boolean;
      }>(["settings"]);
      if (prev) qc.setQueryData(["settings"], { ...prev, settings: s });
      return { prev };
    },
    onError: (e, _s, ctx) => {
      if (ctx?.prev) qc.setQueryData(["settings"], ctx.prev);
      toast({
        title: "Setting not saved",
        description: errorMessage(e),
        tone: "error",
      });
    },
    onSuccess: (data) => qc.setQueryData(["settings"], data),
  });
}

export function useSaveRouting() {
  const { client } = useHarness();
  const qc = useQueryClient();
  const toast = useToast();
  return useMutation({
    mutationFn: (r: RoutingConfig) => client!.updateRouting(r),
    onMutate: async (r) => {
      await qc.cancelQueries({ queryKey: ["routing"] });
      const prev = qc.getQueryData<RoutingConfig>(["routing"]);
      qc.setQueryData(["routing"], r);
      return { prev };
    },
    onError: (e, _r, ctx) => {
      if (ctx?.prev) qc.setQueryData(["routing"], ctx.prev);
      toast({
        title: "Routing not saved",
        description: errorMessage(e),
        tone: "error",
      });
    },
    onSuccess: (data) => qc.setQueryData(["routing"], data),
  });
}
