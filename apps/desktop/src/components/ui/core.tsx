import {
  createContext,
  useContext,
  useId,
  forwardRef,
  type ButtonHTMLAttributes,
  type HTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type TextareaHTMLAttributes,
} from "react";
import clsx from "clsx";
import * as RTooltip from "@radix-ui/react-tooltip";
import { Loader2 } from "lucide-react";

export const cn = clsx;

type Variant = "primary" | "secondary" | "ghost" | "danger" | "outline";
type Size = "xs" | "sm" | "md";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
  icon?: ReactNode;
}

const variants: Record<Variant, string> = {
  primary: "bg-accent text-accent-fg hover:opacity-90 active:opacity-80",
  secondary: "bg-surface-3 text-fg hover:bg-border-strong",
  outline: "border border-border-strong text-fg hover:bg-surface-2",
  ghost: "text-fg-muted hover:text-fg hover:bg-surface-2",
  danger: "border border-border-strong text-fg hover:bg-fg hover:text-bg",
};

const sizes: Record<Size, string> = {
  xs: "h-6 px-2 text-2xs gap-1 rounded-[5px]",
  sm: "h-7 px-2.5 text-xs gap-1.5 rounded-md",
  md: "h-8 px-3 text-[13px] gap-2 rounded-md",
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      variant = "secondary",
      size = "sm",
      loading,
      icon,
      className,
      children,
      disabled,
      ...rest
    },
    ref,
  ) {
    return (
      <button
        ref={ref}
        className={cn(
          "inline-flex shrink-0 items-center justify-center font-medium whitespace-nowrap transition-[background-color,opacity,color] duration-150 select-none disabled:pointer-events-none disabled:opacity-40",
          variants[variant],
          sizes[size],
          className,
        )}
        disabled={disabled || loading}
        aria-busy={loading || undefined}
        {...rest}
      >
        {loading ? (
          <Loader2
            className="size-3.5 animate-[spin_0.8s_linear_infinite]"
            aria-hidden
          />
        ) : (
          icon
        )}
        {children}
      </button>
    );
  },
);

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  size?: "xs" | "sm" | "md";
  active?: boolean;
  side?: "top" | "right" | "bottom" | "left";
}

/** Icon-only button. Always has a tooltip and an accessible label. */
export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  function IconButton(
    { label, size = "sm", active, className, children, side = "top", ...rest },
    ref,
  ) {
    const dim = size === "xs" ? "size-6" : size === "sm" ? "size-7" : "size-8";
    return (
      <Tooltip content={label} side={side}>
        <button
          ref={ref}
          aria-label={label}
          className={cn(
            "inline-flex shrink-0 items-center justify-center rounded-md transition-colors duration-150 disabled:pointer-events-none disabled:opacity-40",
            active
              ? "bg-surface-3 text-fg"
              : "text-fg-subtle hover:bg-surface-2 hover:text-fg",
            dim,
            className,
          )}
          {...rest}
        >
          {children}
        </button>
      </Tooltip>
    );
  },
);

export function TooltipProvider({ children }: { children: ReactNode }) {
  return (
    <RTooltip.Provider delayDuration={350} skipDelayDuration={150}>
      {children}
    </RTooltip.Provider>
  );
}

export function Tooltip({
  content,
  children,
  side = "top",
  align = "center",
}: {
  content: ReactNode;
  children: ReactNode;
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
}) {
  if (content === null || content === undefined || content === "")
    return <>{children}</>;
  return (
    <RTooltip.Root>
      <RTooltip.Trigger asChild>{children}</RTooltip.Trigger>
      <RTooltip.Portal>
        <RTooltip.Content
          side={side}
          align={align}
          sideOffset={6}
          collisionPadding={8}
          className="fade-in z-50 max-w-72 rounded-md border border-border-strong bg-surface-2 px-2 py-1.5 text-xs leading-snug text-fg shadow-panel"
        >
          {content}
        </RTooltip.Content>
      </RTooltip.Portal>
    </RTooltip.Root>
  );
}

export function Panel({
  className,
  children,
  ...rest
}: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("rounded-lg border border-border bg-surface", className)}
      {...rest}
    >
      {children}
    </div>
  );
}

export function PanelHeader({
  title,
  subtitle,
  actions,
  className,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex min-h-10 items-center gap-3 border-b border-border px-3.5 py-2",
        className,
      )}
    >
      <div className="min-w-0 flex-1">
        <h3 className="truncate text-[12.5px] font-medium text-fg">{title}</h3>
        {subtitle && (
          <p className="truncate text-2xs text-fg-subtle">{subtitle}</p>
        )}
      </div>
      {actions && (
        <div className="flex shrink-0 items-center gap-1">{actions}</div>
      )}
    </div>
  );
}

export function Badge({
  children,
  className,
  mono,
}: {
  children: ReactNode;
  className?: string;
  mono?: boolean;
}) {
  return (
    <span
      className={cn(
        "inline-flex h-[18px] shrink-0 items-center gap-1 rounded-[4px] border border-border-strong px-1.5 text-2xs whitespace-nowrap text-fg-muted",
        mono && "font-mono",
        className,
      )}
    >
      {children}
    </span>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded border border-border-strong bg-surface-2 px-1 font-mono text-2xs text-fg-muted">
      {children}
    </kbd>
  );
}

export function Mono({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <span className={cn("font-mono text-[12px] tnum", className)}>
      {children}
    </span>
  );
}

export function Spinner({ className }: { className?: string }) {
  return (
    <Loader2
      className={cn(
        "size-3.5 animate-[spin_0.8s_linear_infinite] text-fg-subtle",
        className,
      )}
      aria-label="Loading"
    />
  );
}

export function Skeleton({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        "animate-[pulse-soft_1.6s_ease-in-out_infinite] rounded bg-surface-3",
        className,
      )}
      aria-hidden
    />
  );
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon?: ReactNode;
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-2 px-6 py-10 text-center",
        className,
      )}
    >
      {icon && <div className="mb-1 text-fg-faint [&_svg]:size-5">{icon}</div>}
      <p className="text-[13px] font-medium text-fg">{title}</p>
      {description && (
        <p className="max-w-sm text-xs text-fg-subtle">{description}</p>
      )}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}

const FieldContext = createContext<{
  id: string;
  description?: string;
  invalid?: boolean;
} | null>(null);
export function useField() {
  return useContext(FieldContext);
}

export const Input = forwardRef<
  HTMLInputElement,
  InputHTMLAttributes<HTMLInputElement> & { mono?: boolean }
>(function Input({ className, mono, ...rest }, ref) {
  const field = useField();
  return (
    <input
      id={field?.id}
      aria-describedby={field?.description}
      aria-invalid={field?.invalid || undefined}
      ref={ref}
      className={cn(
        "h-8 w-full rounded-md border border-border-strong bg-bg px-2.5 text-[13px] text-fg placeholder:text-fg-faint transition-colors outline-none focus:border-fg-subtle disabled:opacity-50",
        mono && "font-mono text-[12px]",
        className,
      )}
      {...rest}
    />
  );
});

export const Textarea = forwardRef<
  HTMLTextAreaElement,
  TextareaHTMLAttributes<HTMLTextAreaElement>
>(function Textarea({ className, ...rest }, ref) {
  const field = useField();
  return (
    <textarea
      id={field?.id}
      aria-describedby={field?.description}
      aria-invalid={field?.invalid || undefined}
      ref={ref}
      className={cn(
        "w-full resize-none rounded-md border border-border-strong bg-bg px-2.5 py-2 text-[13px] text-fg placeholder:text-fg-faint outline-none focus:border-fg-subtle",
        className,
      )}
      {...rest}
    />
  );
});

export function Field({
  label,
  hint,
  children,
  error,
}: {
  label: ReactNode;
  hint?: ReactNode;
  children: ReactNode;
  error?: string | null;
}) {
  const id = useId();
  const description = error || hint ? `${id}-description` : undefined;
  return (
    <FieldContext.Provider value={{ id, description, invalid: !!error }}>
      <div className="flex flex-col gap-1.5">
        <label htmlFor={id} className="text-xs font-medium text-fg-muted">
          {label}
        </label>
        {children}
        {description && (
          <div
            id={description}
            className={cn("text-2xs", error ? "text-fg" : "text-fg-subtle")}
          >
            {error || hint}
          </div>
        )}
      </div>
    </FieldContext.Provider>
  );
}

export function Divider({ className }: { className?: string }) {
  return <div className={cn("h-px bg-border", className)} />;
}

/** Section label used in dense side panels and forms. */
export function Eyebrow({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "text-2xs font-medium tracking-wide text-fg-subtle uppercase",
        className,
      )}
    >
      {children}
    </div>
  );
}
