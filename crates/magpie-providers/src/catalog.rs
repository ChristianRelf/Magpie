//! Built-in model knowledge used when a provider's discovery endpoint does
//! not report a property. Everything derived here is marked `Estimated`:
//! model lineups and prices change, and provider-reported metadata always
//! takes precedence.
//!
//! Prices are first-party list prices per million tokens as known at
//! `CATALOG_DATE`. Unknown prices are left empty rather than guessed.

use magpie_core::*;

pub const CATALOG_DATE: &str = "2026-06-24";

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub tier: QualityTier,
    pub speed: SpeedClass,
    pub capabilities: Capabilities,
    pub context_window: Option<u64>,
    pub max_output: Option<u64>,
    pub pricing: Option<Pricing>,
    pub display_name: Option<String>,
}

fn price(input: f64, output: f64, cached: Option<f64>) -> Option<Pricing> {
    Some(Pricing { input_per_mtok: input, output_per_mtok: output, cached_input_per_mtok: cached, provenance: Provenance::Estimated })
}

fn base_caps() -> Capabilities {
    Capabilities { streaming: true, system_prompt: true, ..Default::default() }
}

/// Parameter count in billions parsed from ids like `llama-3.1-70b`.
fn param_billions(id: &str) -> Option<f64> {
    let bytes = id.as_bytes();
    for (i, w) in id.char_indices() {
        if (w == 'b' || w == 'B') && i > 0 {
            let next_is_alnum = bytes.get(i + 1).map(|c| c.is_ascii_alphanumeric()).unwrap_or(false);
            if next_is_alnum {
                continue;
            }
            let start = id[..i].rfind(|c: char| !(c.is_ascii_digit() || c == '.')).map(|p| p + 1).unwrap_or(0);
            if start < i {
                if let Ok(n) = id[start..i].parse::<f64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Infer metadata for a model. `vendor` is the model family owner
/// (`anthropic`, `openai`, `google`, ...); for aggregators pass the id prefix.
pub fn infer(vendor: &str, model_id: &str) -> CatalogEntry {
    let id = model_id.to_ascii_lowercase();
    let vendor = if vendor == "openrouter" || vendor == "generic" {
        id.split('/').next().unwrap_or(vendor).to_string()
    } else {
        vendor.to_string()
    };
    let bare = id.rsplit('/').next().unwrap_or(&id).to_string();

    if vendor == "anthropic" || bare.contains("claude") || ["opus", "sonnet", "haiku", "fable", "mythos"].contains(&bare.as_str()) {
        return anthropic(&bare);
    }
    if vendor == "google" || bare.starts_with("gemini") || bare.starts_with("gemma") {
        return google(&bare);
    }
    if vendor == "openai" || bare.starts_with("gpt") || bare.starts_with('o') && bare.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return openai(&bare);
    }
    open_model(&bare)
}

fn anthropic(id: &str) -> CatalogEntry {
    let mut caps = base_caps();
    caps.tools = true;
    caps.vision = true;
    caps.structured_output = true;
    caps.reasoning = true;
    let one_m = Some(1_000_000);
    let (tier, speed, ctx, out, pricing, name) = if id.contains("fable") || id.contains("mythos") {
        (QualityTier::Frontier, SpeedClass::Slow, one_m, Some(128_000), price(10.0, 50.0, Some(0.25)), "Claude Fable")
    } else if id.contains("opus-5-5") {
        (QualityTier::Frontier, SpeedClass::Medium, one_m, Some(128_000), price(4.0, 20.0, Some(0.20)), "Claude Opus 5.5")
    } else if id.contains("opus") {
        (QualityTier::Frontier, SpeedClass::Medium, one_m, Some(128_000), price(5.0, 25.0, Some(0.5)), "Claude Opus")
    } else if id.contains("sonnet-5") {
        (QualityTier::High, SpeedClass::Medium, one_m, Some(128_000), price(2.0, 10.0, Some(0.2)), "Claude Sonnet 5")
    } else if id.contains("sonnet") {
        (QualityTier::High, SpeedClass::Medium, one_m, Some(128_000), price(3.0, 15.0, Some(0.3)), "Claude Sonnet")
    } else if id.contains("haiku") {
        caps.reasoning = id.contains("4-5") || id.contains("4.5");
        (QualityTier::Standard, SpeedClass::Fast, Some(200_000), Some(64_000), price(1.0, 5.0, Some(0.1)), "Claude Haiku")
    } else {
        (QualityTier::High, SpeedClass::Medium, Some(200_000), None, None, "Claude")
    };
    CatalogEntry { tier, speed, capabilities: caps, context_window: ctx, max_output: out, pricing, display_name: Some(name.into()) }
}

fn google(id: &str) -> CatalogEntry {
    let mut caps = base_caps();
    caps.tools = true;
    caps.vision = true;
    caps.structured_output = true;
    caps.reasoning = !id.contains("1.5") && !id.contains("2.0");
    let (tier, speed) = if id.contains("ultra") || id.contains("deep-think") {
        (QualityTier::Frontier, SpeedClass::Slow)
    } else if id.contains("pro") {
        (QualityTier::High, SpeedClass::Medium)
    } else if id.contains("lite") || id.starts_with("gemma") {
        (QualityTier::Light, SpeedClass::Fast)
    } else if id.contains("flash") {
        (QualityTier::Standard, SpeedClass::Fast)
    } else {
        (QualityTier::Standard, SpeedClass::Medium)
    };
    if id.starts_with("gemma") {
        caps.tools = false;
        caps.structured_output = false;
    }
    CatalogEntry { tier, speed, capabilities: caps, context_window: Some(1_048_576), max_output: Some(65_536), pricing: None, display_name: None }
}

fn openai(id: &str) -> CatalogEntry {
    let mut caps = base_caps();
    caps.tools = true;
    caps.structured_output = true;
    caps.vision = id.starts_with("gpt-4o") || id.starts_with("gpt-4.1") || id.starts_with("gpt-5") || id.starts_with("gpt-6") || id.starts_with("o3") || id.starts_with("o4");
    caps.reasoning = id.starts_with('o') || id.starts_with("gpt-5") || id.starts_with("gpt-6") || id.contains("codex");
    let light = id.contains("nano") || id.contains("luna") || id.contains("mini") || id.contains("3.5");
    let frontier = id.ends_with("-pro") || id.contains("astra") || id.contains("-pro-");
    let (tier, speed) = if frontier {
        (QualityTier::Frontier, SpeedClass::Slow)
    } else if light {
        (if id.contains("nano") { QualityTier::Light } else { QualityTier::Standard }, SpeedClass::Fast)
    } else if id.starts_with("gpt-5") || id.starts_with("gpt-6") || id.starts_with("o3") || id.contains("sol") {
        (QualityTier::High, SpeedClass::Medium)
    } else {
        (QualityTier::Standard, SpeedClass::Medium)
    };
    let ctx = if id.starts_with("gpt-4.1") { Some(1_047_576) } else if id.starts_with("gpt-5") || id.starts_with("gpt-6") { Some(400_000) } else { Some(128_000) };
    CatalogEntry { tier, speed, capabilities: caps, context_window: ctx, max_output: None, pricing: None, display_name: None }
}

fn open_model(id: &str) -> CatalogEntry {
    let mut caps = base_caps();
    caps.tools = id.contains("instruct") || id.contains("llama-3") || id.contains("llama3") || id.contains("qwen") || id.contains("mistral")
        || id.contains("deepseek") || id.contains("gpt-oss") || id.contains("kimi") || id.contains("glm") || id.contains("command");
    caps.vision = id.contains("vision") || id.contains("-vl") || id.contains("llava") || id.contains("pixtral") || id.contains("gemma3");
    caps.reasoning = id.contains("reasoner") || id.contains("r1") || id.contains("thinking") || id.contains("qwq") || id.contains("gpt-oss");
    caps.structured_output = caps.tools;
    let params = param_billions(id);
    let (tier, speed) = match params {
        Some(p) if p <= 10.0 => (QualityTier::Light, SpeedClass::Fast),
        Some(p) if p <= 40.0 => (QualityTier::Standard, SpeedClass::Medium),
        Some(_) => (QualityTier::High, SpeedClass::Medium),
        None if id.contains("large") || id.contains("deepseek-chat") || id.contains("reasoner") => (QualityTier::High, SpeedClass::Medium),
        None if id.contains("small") || id.contains("tiny") || id.contains("mini") => (QualityTier::Light, SpeedClass::Fast),
        None => (QualityTier::Standard, SpeedClass::Medium),
    };
    CatalogEntry { tier, speed, capabilities: caps, context_window: None, max_output: None, pricing: None, display_name: None }
}

/// Whether a model id from an OpenAI-style listing is a text generation
/// model (filters out embeddings, audio, image and moderation models).
pub fn is_text_model(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    const EXCLUDE: &[&str] = &[
        "embed", "whisper", "tts", "dall-e", "davinci", "babbage", "moderation", "transcribe", "audio", "realtime",
        "image", "sora", "guard", "rerank", "search-preview", "computer-use", "distil-whisper", "playai", "ocr",
    ];
    !EXCLUDE.iter().any(|x| id.contains(x))
}

/// Human-friendly name from a model id when the provider gives none.
pub fn prettify(id: &str) -> String {
    let bare = id.rsplit('/').next().unwrap_or(id);
    let mut out = String::new();
    for (i, part) in bare.split(['-', '_']).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let lower = part.to_ascii_lowercase();
        if lower == "gpt" || lower == "gpt5" {
            out.push_str(&part.to_ascii_uppercase());
        } else if part.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            out.push_str(part);
        } else {
            let mut c = part.chars();
            if let Some(f) = c.next() {
                out.extend(f.to_uppercase());
                out.push_str(c.as_str());
            }
        }
    }
    out
}

/// Build a discovered model from catalog knowledge only.
pub fn discovered_from_catalog(vendor: &str, model_id: &str, display: Option<String>) -> DiscoveredModel {
    let e = infer(vendor, model_id);
    DiscoveredModel {
        model_id: model_id.to_string(),
        display_name: display.or(e.display_name.clone()).unwrap_or_else(|| prettify(model_id)),
        description: None,
        context_window: e.context_window,
        max_output_tokens: e.max_output,
        capabilities: e.capabilities,
        tier: e.tier,
        speed: e.speed,
        pricing: e.pricing,
        metadata_provenance: Provenance::Estimated,
        is_default: false,
        reasoning_efforts: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_tiers() {
        assert_eq!(infer("anthropic", "claude-opus-5").tier, QualityTier::Frontier);
        assert_eq!(infer("anthropic", "claude-haiku-4-5").speed, SpeedClass::Fast);
        assert_eq!(infer("anthropic", "claude-sonnet-5").pricing.unwrap().input_per_mtok, 2.0);
        assert_eq!(infer("anthropic", "sonnet").tier, QualityTier::High);
    }

    #[test]
    fn open_model_sizes() {
        assert_eq!(param_billions("llama-3.1-8b-instant"), Some(8.0));
        assert_eq!(param_billions("llama-3.3-70b-versatile"), Some(70.0));
        assert_eq!(infer("groq", "llama-3.1-8b-instant").tier, QualityTier::Light);
        assert_eq!(infer("groq", "llama-3.3-70b-versatile").tier, QualityTier::High);
    }

    #[test]
    fn unknown_prices_are_not_invented() {
        assert!(infer("openai", "gpt-6-astra").pricing.is_none());
        assert!(infer("ollama", "qwen3:14b").pricing.is_none());
    }

    #[test]
    fn filters_non_text_models() {
        assert!(!is_text_model("text-embedding-3-large"));
        assert!(!is_text_model("whisper-large-v3"));
        assert!(is_text_model("gpt-4.1-mini"));
    }

    #[test]
    fn pretty_names() {
        assert_eq!(prettify("gemini-2.5-flash"), "Gemini 2.5 Flash");
        assert_eq!(prettify("gpt-6-astra"), "GPT 6 Astra");
    }
}
