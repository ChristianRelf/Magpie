import type { ProviderKind } from "@magpie/sdk";
import { CodeXml } from "lucide-react";
import {
  siAnthropic,
  siClaude,
  siDeepseek,
  siGooglegemini,
  siLmstudio,
  siMistralai,
  siOllama,
  siOpenrouter,
} from "simple-icons";
import { cn } from "./core";

type ProviderIcon = { path: string; viewBox?: string };

// Simple Icons does not currently include OpenAI or Groq. These paths come
// from the providers' own repositories; geometry is preserved in monochrome.
// https://github.com/openai/openai-assistants-quickstart/blob/06fc2d444a5d41b574082080f4c7b2e48156b84f/public/openai.svg
const OPENAI: ProviderIcon = {
  viewBox: "0 0 32 32",
  path: "M29.71,13.09A8.09,8.09,0,0,0,20.34,2.68a8.08,8.08,0,0,0-13.7,2.9A8.08,8.08,0,0,0,2.3,18.9,8,8,0,0,0,3,25.45a8.08,8.08,0,0,0,8.69,3.87,8,8,0,0,0,6,2.68,8.09,8.09,0,0,0,7.7-5.61,8,8,0,0,0,5.33-3.86A8.09,8.09,0,0,0,29.71,13.09Zm-12,16.82a6,6,0,0,1-3.84-1.39l.19-.11,6.37-3.68a1,1,0,0,0,.53-.91v-9l2.69,1.56a.08.08,0,0,1,.05.07v7.44A6,6,0,0,1,17.68,29.91ZM4.8,24.41a6,6,0,0,1-.71-4l.19.11,6.37,3.68a1,1,0,0,0,1,0l7.79-4.49V22.8a.09.09,0,0,1,0,.08L13,26.6A6,6,0,0,1,4.8,24.41ZM3.12,10.53A6,6,0,0,1,6.28,7.9v7.57a1,1,0,0,0,.51.9l7.75,4.47L11.85,22.4a.14.14,0,0,1-.09,0L5.32,18.68a6,6,0,0,1-2.2-8.18Zm22.13,5.14-7.78-4.52L20.16,9.6a.08.08,0,0,1,.09,0l6.44,3.72a6,6,0,0,1-.9,10.81V16.56A1.06,1.06,0,0,0,25.25,15.67Zm2.68-4-.19-.12-6.36-3.7a1,1,0,0,0-1.05,0l-7.78,4.49V9.2a.09.09,0,0,1,0-.09L19,5.4a6,6,0,0,1,8.91,6.21ZM11.08,17.15,8.38,15.6a.14.14,0,0,1-.05-.08V8.1a6,6,0,0,1,9.84-4.61L18,3.6,11.61,7.28a1,1,0,0,0-.53.91ZM12.54,14,16,12l3.47,2v4L16,20l-3.47-2Z",
};

// https://github.com/groq/groq-appgen/blob/9b82e8576118978df20cd9e4d22a9ae56c5e8160/public/Groq_Bolt.svg
const GROQ: ProviderIcon = {
  viewBox: "0 0 26 33",
  path: "M17.4481 3.34906L3.63475 17.7685L12.1802 19.9582L8.35182 30.9499L22.1652 16.5305L13.6197 14.3408L17.4481 3.34906Z",
};

// Named imports keep the remaining Simple Icons catalogue out of the bundle.
const PROVIDER_ICONS: Record<ProviderKind, ProviderIcon | null> = {
  claude_code: siClaude,
  anthropic: siAnthropic,
  codex_cli: OPENAI,
  openai: OPENAI,
  gemini_cli: siGooglegemini,
  gemini: siGooglegemini,
  open_router: siOpenrouter,
  groq: GROQ,
  mistral: siMistralai,
  deep_seek: siDeepseek,
  ollama: siOllama,
  lm_studio: siLmstudio,
  open_ai_compatible: null,
};

export function ProviderMark({
  kind,
  size = 28,
  className,
}: {
  kind: ProviderKind | string;
  size?: number;
  className?: string;
}) {
  const icon = Object.hasOwn(PROVIDER_ICONS, kind) ? PROVIDER_ICONS[kind as ProviderKind] : null;
  const glyphSize = size * 0.6;
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-md border border-border-strong bg-surface-2 text-fg",
        className,
      )}
      style={{ width: size, height: size }}
      aria-hidden
    >
      {icon ? (
        <svg
          viewBox={icon.viewBox ?? "0 0 24 24"}
          width={glyphSize}
          height={glyphSize}
          fill="currentColor"
          focusable="false"
        >
          <path d={icon.path} />
        </svg>
      ) : (
        <CodeXml size={glyphSize} aria-hidden focusable="false" />
      )}
    </span>
  );
}

/** The Magpie mark: a diamond split into black and white plumage. */
export function MagpieMark({ size = 20, className }: { size?: number; className?: string }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} className={className} aria-hidden>
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
