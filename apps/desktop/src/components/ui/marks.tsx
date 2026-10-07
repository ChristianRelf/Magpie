import type { ProviderKind } from "@magpie/sdk";
import { cn } from "./core";

/**
 * Monochrome identity marks. These are original geometric glyphs that
 * evoke each vendor family without reproducing trademarks.
 */
const VENDOR: Record<ProviderKind, string> = {
  claude_code: "anthropic",
  anthropic: "anthropic",
  codex_cli: "openai",
  openai: "openai",
  gemini_cli: "google",
  gemini: "google",
  open_router: "router",
  groq: "groq",
  mistral: "mistral",
  deep_seek: "deepseek",
  ollama: "local",
  lm_studio: "local",
  open_ai_compatible: "generic",
};

function Glyph({ vendor }: { vendor: string }) {
  switch (vendor) {
    case "anthropic":
      // Radiating burst.
      return (
        <g stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
          {[0, 30, 60, 90, 120, 150].map((a) => (
            <line
              key={a}
              x1="8"
              y1="3.2"
              x2="8"
              y2="12.8"
              transform={`rotate(${a} 8 8)`}
            />
          ))}
        </g>
      );
    case "openai":
      // Interlocking hexagonal knot.
      return (
        <g
          fill="none"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinejoin="round"
        >
          <path d="M8 2.6 12.7 5.3v5.4L8 13.4 3.3 10.7V5.3Z" />
          <path d="M8 5.2 10.4 6.6v2.8L8 10.8 5.6 9.4V6.6Z" />
        </g>
      );
    case "google":
      // Four-point sparkle.
      return (
        <path
          d="M8 2c.5 3.2 2.8 5.5 6 6-3.2.5-5.5 2.8-6 6-.5-3.2-2.8-5.5-6-6 3.2-.5 5.5-2.8 6-6Z"
          fill="currentColor"
        />
      );
    case "router":
      return (
        <g
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        >
          <path d="M2.5 8h4l3-3.5H13M6.5 8l3 3.5H13" />
          <circle cx="13" cy="4.5" r="0.9" fill="currentColor" />
          <circle cx="13" cy="11.5" r="0.9" fill="currentColor" />
        </g>
      );
    case "groq":
      return (
        <g
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
        >
          <path
            d="M9 2.5 4.5 9H8l-1 4.5L11.5 7H8Z"
            fill="currentColor"
            stroke="none"
          />
        </g>
      );
    case "mistral":
      return (
        <g fill="currentColor">
          {[0, 1, 2, 3].map((i) => (
            <rect
              key={i}
              x={2.5 + i * 2.9}
              y={3 + (i % 2) * 2}
              width="2.2"
              height={10 - (i % 2) * 4}
              rx="0.4"
            />
          ))}
        </g>
      );
    case "deepseek":
      return (
        <path
          d="M2.5 9.5c2-4 7-6 11-4-1.5.5-2.5 1.8-2.5 3.5 0 2-1.7 3.5-4 3.5-2.2 0-3.8-1.2-4.5-3Z"
          fill="currentColor"
        />
      );
    case "local":
      return (
        <g fill="none" stroke="currentColor" strokeWidth="1.4">
          <rect x="2.5" y="3.5" width="11" height="7.5" rx="1.2" />
          <path d="M5.5 13.2h5" strokeLinecap="round" />
        </g>
      );
    default:
      return (
        <g
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        >
          <path d="M5.5 4.5 2.5 8l3 3.5M10.5 4.5l3 3.5-3 3.5" />
        </g>
      );
  }
}

export function ProviderMark({
  kind,
  size = 28,
  className,
}: {
  kind: ProviderKind | string;
  size?: number;
  className?: string;
}) {
  const vendor = VENDOR[kind as ProviderKind] ?? "generic";
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-md border border-border-strong bg-surface-2 text-fg",
        className,
      )}
      style={{ width: size, height: size }}
      aria-hidden
    >
      <svg viewBox="0 0 16 16" width={size * 0.56} height={size * 0.56}>
        <Glyph vendor={vendor} />
      </svg>
    </span>
  );
}

/** The Magpie mark: a diamond split into black and white plumage. */
export function MagpieMark({
  size = 20,
  className,
}: {
  size?: number;
  className?: string;
}) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      className={className}
      aria-hidden
    >
      <path
        d="M12 2.5 21.5 12 12 21.5 2.5 12Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinejoin="round"
      />
      <path d="M12 2.5 21.5 12 12 21.5Z" fill="currentColor" />
      <circle cx="8.6" cy="10.4" r="1.25" fill="currentColor" />
    </svg>
  );
}
