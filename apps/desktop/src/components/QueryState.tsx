import { AlertCircle } from "lucide-react";
import { Button, EmptyState, Spinner } from "./ui/core";
import { errorMessage } from "./ui/feedback";
export function QueryState({
  pending,
  error,
  retry,
}: {
  pending?: boolean;
  error?: unknown;
  retry?: () => unknown;
}) {
  if (error)
    return (
      <EmptyState
        icon={<AlertCircle />}
        title="Could not load this view"
        description={errorMessage(error)}
        action={retry && <Button onClick={() => retry()}>Try again</Button>}
      />
    );
  if (pending)
    return (
      <div className="flex justify-center p-12" role="status">
        <Spinner />
        <span className="sr-only">Loading</span>
      </div>
    );
  return null;
}
