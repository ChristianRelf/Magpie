//! Cheap, deterministic task classification from request metadata and
//! text heuristics. No model calls are made here.

use magpie_core::*;

const CODE_MARKERS: &[&str] = &[
    "```", "fn ", "def ", "class ", "function ", "const ", "let ", "import ", "#include", "public static", "=> {",
    "func ", "struct ", "impl ", "select * from", "<div", "package ", "console.log", "println!", ".rs", ".py", ".ts",
    ".tsx", ".js", ".go", ".java", ".cpp", ".c:", "cargo ", "npm ", "pip ",
];
const DEBUG_MARKERS: &[&str] = &[
    "bug", "error", "fix", "debug", "traceback", "exception", "stack trace", "panic", "segfault", "crash", "failing",
    "why does", "why is", "fails", "broken", "wrong output", "throws", "undefined behavior",
    "doesn't work", "does not work", "not working", "unexpected", "undefined is not", "null pointer", "error[e",
];
const REPO_MARKERS: &[&str] = &["repository", "repo ", "codebase", "code base", "monorepo", "across files", "multiple files", "project structure"];
const MATH_MARKERS: &[&str] = &[
    "prove", "proof", "theorem", "integral", "derivative", "equation", "calculate", "probability", "lemma", "solve for",
    "matrix", "eigen", "optimi", "combinator", "∫", "∑",
];
const SUMMARY_MARKERS: &[&str] = &["summarize", "summarise", "summary", "tl;dr", "tldr", "key points", "condense", "recap"];
const PLAN_MARKERS: &[&str] = &["plan", "strategy", "roadmap", "design a", "architecture", "step-by-step", "trade-off", "tradeoff", "proposal"];
const EXTRACT_MARKERS: &[&str] = &["extract", "parse", "as json", "into json", "fields", "structured", "table of", "csv"];
const CODEGEN_MARKERS: &[&str] = &["write", "implement", "create", "generate", "refactor", "build", "add a", "convert"];
const DEPTH_MARKERS: &[&str] = &["complex", "in depth", "in-depth", "thorough", "comprehensive", "carefully", "detailed", "rigorous", "edge cases"];

fn count(text: &str, markers: &[&str]) -> usize {
    markers.iter().filter(|m| text.contains(*m)).count()
}

pub fn classify(req: &ExecRequest) -> Classification {
    let all = req.all_text();
    let last = req.last_user_text().to_lowercase();
    let lower_all = if all.len() > 200_000 { all[..200_000].to_lowercase() } else { all.to_lowercase() };
    let images = req.messages.iter().flat_map(|m| m.content.iter()).filter(|p| matches!(p, ContentPart::Image { .. })).count() as u64;
    let est_input = estimate_tokens(&all) + images * 1_200 + req.tools.len() as u64 * 150;

    let mut required = Capabilities::default();
    let mut signals = Vec::new();
    if !req.tools.is_empty() || req.messages.iter().any(|m| m.role == Role::Tool || !m.tool_calls.is_empty()) {
        required.tools = true;
        signals.push(format!("{} tool definition(s)", req.tools.len()));
    }
    if matches!(req.response_format, Some(ResponseFormat::JsonSchema { .. })) {
        required.structured_output = true;
        signals.push("JSON schema output".into());
    }
    if images > 0 {
        required.vision = true;
        signals.push(format!("{images} image input(s)"));
    }
    if req.agent.is_some() {
        required.agentic = true;
        signals.push("Agentic execution requested".into());
    }

    let code = count(&lower_all, CODE_MARKERS);
    let debug = count(&last, DEBUG_MARKERS);
    let repo = count(&last, REPO_MARKERS);
    let math = count(&last, MATH_MARKERS);
    let summary = count(&last, SUMMARY_MARKERS);
    let plan = count(&last, PLAN_MARKERS);
    let extract = count(&last, EXTRACT_MARKERS);
    let codegen = count(&last, CODEGEN_MARKERS);
    let depth = count(&last, DEPTH_MARKERS);

    let (task, explicit) = if let Some(t) = req.task_type {
        signals.push("Task type provided by client".into());
        (t, true)
    } else if required.tools {
        (TaskClass::ToolExecution, false)
    } else if req.response_format.is_some() || (extract >= 2) {
        (TaskClass::DataExtraction, false)
    } else if est_input > 60_000 {
        signals.push(format!("~{}k input tokens", est_input / 1000));
        if code >= 3 || repo > 0 { (TaskClass::RepositoryAnalysis, false) } else { (TaskClass::LargeContext, false) }
    } else if repo > 0 || (req.agent.is_some() && code > 0) {
        (TaskClass::RepositoryAnalysis, false)
    } else if code > 0 && debug > 0 {
        (TaskClass::Debugging, false)
    } else if summary > 0 {
        (TaskClass::Summarisation, false)
    } else if math > 0 && code == 0 {
        (TaskClass::MathReasoning, false)
    } else if code > 0 || (codegen > 0 && (last.contains("code") || last.contains("script") || last.contains("program"))) {
        (TaskClass::CodeGeneration, false)
    } else if debug >= 2 {
        (TaskClass::Debugging, false)
    } else if plan > 0 {
        (TaskClass::Planning, false)
    } else if extract > 0 {
        (TaskClass::DataExtraction, false)
    } else if last.chars().count() < 240 {
        (TaskClass::SimpleQuestion, false)
    } else {
        (TaskClass::General, false)
    };
    if !explicit && code > 0 {
        signals.push("Code detected".into());
    }

    let effort_high = matches!(req.reasoning_effort.as_deref(), Some("high" | "xhigh" | "max"));
    let complexity = if est_input > 30_000
        || effort_high
        || depth > 0
        || (matches!(task, TaskClass::Planning | TaskClass::MathReasoning | TaskClass::RepositoryAnalysis) && last.len() > 400)
        || task == TaskClass::LargeContext
    {
        Complexity::High
    } else if task == TaskClass::SimpleQuestion
        || (last.len() < 300 && !matches!(task, TaskClass::MathReasoning | TaskClass::Planning | TaskClass::Debugging | TaskClass::RepositoryAnalysis))
    {
        Complexity::Low
    } else {
        Complexity::Medium
    };
    if depth > 0 {
        signals.push("Depth requested".into());
    }

    let est_output = req.max_output_tokens.map(u64::from).unwrap_or(match task {
        TaskClass::SimpleQuestion => 300,
        TaskClass::Summarisation | TaskClass::DataExtraction | TaskClass::ToolExecution => 600,
        TaskClass::CodeGeneration | TaskClass::Debugging => 1_500,
        TaskClass::Planning | TaskClass::MathReasoning | TaskClass::RepositoryAnalysis => 2_000,
        _ => 800,
    });

    Classification { task, complexity, required, estimated_input_tokens: est_input, estimated_output_tokens: est_output, explicit, signals }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(prompt: &str) -> Classification {
        classify(&ExecRequest::simple(prompt))
    }

    #[test]
    fn simple_question() {
        let r = c("What is the capital of France?");
        assert_eq!(r.task, TaskClass::SimpleQuestion);
        assert_eq!(r.complexity, Complexity::Low);
    }

    #[test]
    fn debugging_with_code() {
        let r = c("Why does this fail?\n```rust\nfn main() { let x: i32 = \"a\"; }\n```\nerror[E0308]: mismatched types");
        assert_eq!(r.task, TaskClass::Debugging);
    }

    #[test]
    fn panics_are_debugging() {
        let r = c("Why does this panic? fn main() { let v: Vec<i32> = vec![]; v[0]; }");
        assert_eq!(r.task, TaskClass::Debugging);
        assert_ne!(r.complexity, Complexity::Low);
    }

    #[test]
    fn code_generation() {
        assert_eq!(c("Write a Python script that renames files by date").task, TaskClass::CodeGeneration);
    }

    #[test]
    fn summarisation_and_math_and_plan() {
        assert_eq!(c("Summarize the following meeting notes: ...").task, TaskClass::Summarisation);
        assert_eq!(c("Prove that the square root of 2 is irrational").task, TaskClass::MathReasoning);
        assert_eq!(c("Create a migration plan and roadmap for moving our services to a new region").task, TaskClass::Planning);
    }

    #[test]
    fn large_context_is_high_complexity() {
        let r = c(&"lorem ipsum dolor ".repeat(20_000));
        assert_eq!(r.task, TaskClass::LargeContext);
        assert_eq!(r.complexity, Complexity::High);
        assert!(r.estimated_input_tokens > 60_000);
    }

    #[test]
    fn capabilities_from_request() {
        let mut r = ExecRequest::simple("look this up");
        r.tools.push(ToolDefinition { name: "search".into(), description: None, parameters: serde_json::json!({}) });
        r.messages[0].content.push(ContentPart::Image { media_type: None, data: Some("AA".into()), url: None });
        let cl = classify(&r);
        assert!(cl.required.tools && cl.required.vision);
        assert_eq!(cl.task, TaskClass::ToolExecution);
    }

    #[test]
    fn explicit_task_type_wins() {
        let mut r = ExecRequest::simple("hello");
        r.task_type = Some(TaskClass::Planning);
        let cl = classify(&r);
        assert_eq!(cl.task, TaskClass::Planning);
        assert!(cl.explicit);
    }
}
