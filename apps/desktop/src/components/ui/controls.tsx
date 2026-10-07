import type { ReactNode } from "react";
import * as RSwitch from "@radix-ui/react-switch";
import * as RSelect from "@radix-ui/react-select";
import * as RDialog from "@radix-ui/react-dialog";
import * as RSlider from "@radix-ui/react-slider";
import { Check, ChevronDown, X } from "lucide-react";
import { cn, useField } from "./core";

export function Switch({
  checked,
  onCheckedChange,
  disabled,
  label,
}: {
  checked: boolean;
  onCheckedChange: (v: boolean) => void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <RSwitch.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={label}
      className="relative inline-flex h-[18px] w-8 shrink-0 cursor-pointer items-center rounded-full border border-border-strong bg-surface-3 transition-colors duration-150 data-[state=checked]:border-fg data-[state=checked]:bg-fg disabled:cursor-not-allowed disabled:opacity-40"
    >
      <RSwitch.Thumb className="block size-3 translate-x-[2px] rounded-full bg-fg-subtle transition-transform duration-150 ease-out-soft data-[state=checked]:translate-x-[15px] data-[state=checked]:bg-bg" />
    </RSwitch.Root>
  );
}

/** A settings row: label + description on the left, control on the right. */
export function SettingRow({
  title,
  description,
  children,
  disabled,
}: {
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  disabled?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex items-center justify-between gap-6 px-4 py-3",
        disabled && "opacity-50",
      )}
    >
      <div className="min-w-0">
        <div className="text-[13px] text-fg">{title}</div>
        {description && (
          <div className="mt-0.5 text-xs text-fg-subtle">{description}</div>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

export interface SegmentOption<T extends string> {
  value: T;
  label: ReactNode;
  title?: string;
}

/** Compact segmented control. */
export function Segmented<T extends string>({
  value,
  onChange,
  options,
  size = "sm",
  label,
  className,
}: {
  value: T;
  onChange: (v: T) => void;
  options: SegmentOption<T>[];
  size?: "xs" | "sm";
  label: string;
  className?: string;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className={cn(
        "inline-flex rounded-md border border-border bg-bg-subtle p-0.5",
        className,
      )}
    >
      {options.map((o) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            role="radio"
            aria-checked={active}
            title={o.title}
            onClick={() => onChange(o.value)}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-[5px] font-medium whitespace-nowrap transition-colors duration-150",
              size === "xs" ? "h-5 px-1.5 text-2xs" : "h-6 px-2.5 text-xs",
              active
                ? "bg-surface-3 text-fg shadow-[inset_0_0_0_1px_var(--border-strong)]"
                : "text-fg-subtle hover:text-fg",
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

export interface SelectOption {
  value: string;
  label: ReactNode;
  hint?: ReactNode;
}

export function Select({
  value,
  onChange,
  options,
  placeholder,
  label,
  className,
  size = "md",
}: {
  value: string | undefined;
  onChange: (v: string) => void;
  options: SelectOption[];
  placeholder?: string;
  label: string;
  className?: string;
  size?: "sm" | "md";
}) {
  const field = useField();
  return (
    <RSelect.Root value={value} onValueChange={onChange}>
      <RSelect.Trigger
        id={field?.id}
        aria-describedby={field?.description}
        aria-label={label}
        className={cn(
          "inline-flex w-full items-center justify-between gap-2 rounded-md border border-border-strong bg-bg px-2.5 text-left text-fg outline-none focus:border-fg-subtle data-[placeholder]:text-fg-faint",
          size === "sm" ? "h-7 text-xs" : "h-8 text-[13px]",
          className,
        )}
      >
        <span className="truncate">
          <RSelect.Value placeholder={placeholder} />
        </span>
        <RSelect.Icon>
          <ChevronDown className="size-3.5 text-fg-subtle" />
        </RSelect.Icon>
      </RSelect.Trigger>
      <RSelect.Portal>
        <RSelect.Content
          position="popper"
          sideOffset={4}
          className="fade-in z-50 max-h-80 min-w-[var(--radix-select-trigger-width)] overflow-hidden rounded-md border border-border-strong bg-surface-2 shadow-panel"
        >
          <RSelect.Viewport className="p-1">
            {options.map((o) => (
              <RSelect.Item
                key={o.value}
                value={o.value}
                className="relative flex cursor-default items-center gap-2 rounded-[5px] py-1.5 pr-2 pl-7 text-[13px] text-fg outline-none select-none data-[highlighted]:bg-surface-3"
              >
                <RSelect.ItemIndicator className="absolute left-2">
                  <Check className="size-3.5" strokeWidth={2.5} />
                </RSelect.ItemIndicator>
                <RSelect.ItemText>{o.label}</RSelect.ItemText>
                {o.hint && (
                  <span className="ml-auto pl-3 text-2xs text-fg-subtle">
                    {o.hint}
                  </span>
                )}
              </RSelect.Item>
            ))}
          </RSelect.Viewport>
        </RSelect.Content>
      </RSelect.Portal>
    </RSelect.Root>
  );
}

export function Slider({
  value,
  onChange,
  min,
  max,
  step,
  label,
}: {
  value: number;
  onChange: (v: number) => void;
  min: number;
  max: number;
  step: number;
  label: string;
}) {
  return (
    <RSlider.Root
      value={[value]}
      onValueChange={(v) => onChange(v[0])}
      min={min}
      max={max}
      step={step}
      className="relative flex h-5 w-full touch-none items-center select-none"
      aria-label={label}
    >
      <RSlider.Track className="relative h-[3px] grow rounded-full bg-surface-3">
        <RSlider.Range className="absolute h-full rounded-full bg-fg-muted" />
      </RSlider.Track>
      <RSlider.Thumb
        className="block size-3.5 rounded-full border-2 border-bg bg-fg shadow outline-none focus-visible:ring-2 focus-visible:ring-ring"
        aria-label={label}
      />
    </RSlider.Root>
  );
}

export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  width = "w-[480px]",
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  width?: string;
}) {
  return (
    <RDialog.Root open={open} onOpenChange={onOpenChange}>
      <RDialog.Portal>
        <RDialog.Overlay className="fade-in fixed inset-0 z-40 bg-[var(--overlay)] backdrop-blur-[2px]" />
        <RDialog.Content
          className={cn(
            "rise-in fixed top-1/2 left-1/2 z-50 flex max-h-[86vh] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 flex-col rounded-xl border border-border-strong bg-surface shadow-panel outline-none",
            width,
          )}
        >
          <div className="flex items-start gap-3 px-5 pt-4 pb-3">
            <div className="min-w-0 flex-1">
              <RDialog.Title className="text-[14px] font-semibold text-fg">
                {title}
              </RDialog.Title>
              {description ? (
                <RDialog.Description className="mt-1 text-xs text-fg-subtle">
                  {description}
                </RDialog.Description>
              ) : (
                <RDialog.Description className="sr-only">
                  {typeof title === "string" ? title : "Dialog"}
                </RDialog.Description>
              )}
            </div>
            <RDialog.Close
              className="-mt-0.5 -mr-1.5 rounded-md p-1 text-fg-subtle hover:bg-surface-2 hover:text-fg"
              aria-label="Close"
            >
              <X className="size-4" />
            </RDialog.Close>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-4">
            {children}
          </div>
          {footer && (
            <div className="flex items-center justify-end gap-2 border-t border-border px-5 py-3">
              {footer}
            </div>
          )}
        </RDialog.Content>
      </RDialog.Portal>
    </RDialog.Root>
  );
}

/** Right-side inspector drawer. */
export function Drawer({
  open,
  onOpenChange,
  title,
  children,
  actions,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  title: ReactNode;
  children: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <RDialog.Root open={open} onOpenChange={onOpenChange}>
      <RDialog.Portal>
        <RDialog.Overlay className="fade-in fixed inset-0 z-40 bg-[var(--overlay)]" />
        <RDialog.Content className="fixed top-0 right-0 bottom-0 z-50 flex w-[560px] max-w-[92vw] animate-[drawer-in_200ms_var(--ease-out-soft)] flex-col border-l border-border-strong bg-surface shadow-panel outline-none">
          <div className="flex h-12 shrink-0 items-center gap-2 border-b border-border px-4">
            <RDialog.Title className="min-w-0 flex-1 truncate text-[13px] font-semibold">
              {title}
            </RDialog.Title>
            <RDialog.Description className="sr-only">
              Details
            </RDialog.Description>
            {actions}
            <RDialog.Close
              className="rounded-md p-1 text-fg-subtle hover:bg-surface-2 hover:text-fg"
              aria-label="Close"
            >
              <X className="size-4" />
            </RDialog.Close>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
        </RDialog.Content>
      </RDialog.Portal>
    </RDialog.Root>
  );
}
