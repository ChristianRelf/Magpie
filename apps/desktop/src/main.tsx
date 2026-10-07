import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "./styles.css";
import { App } from "./App";
import { HarnessProvider } from "./lib/harness";
import { ToastProvider } from "./components/ui/feedback";
import { TooltipProvider } from "./components/ui/core";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <TooltipProvider>
          <HarnessProvider>
            <App />
          </HarnessProvider>
        </TooltipProvider>
      </ToastProvider>
    </QueryClientProvider>
  </StrictMode>,
);
