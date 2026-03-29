#![cfg(feature = "container-tests")]
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rehydration_proto::v1beta1::{
    GetContextRequest, RehydrationMode, RenderedContext, ResolutionTier,
};
use rehydration_testkit::{
    Domain, EvaluationGroundTruth, GeneratedNode, GeneratedSeed, GraphSeedConfig,
    LlmEvaluatorConfig, LlmProvider, NoiseMode, PromptConfig, RelationMix, calibrate_judge,
    evaluate_with_config, generate_seed,
    seed_publisher::seed_to_projection_events,
};
use rehydration_tests_shared::containers::connect_nats_with_retry;
use rehydration_tests_shared::fixtures::TestFixture;
use serde::Serialize;
use tokio::time::sleep;

const SUBJECT_PREFIX: &str = "rehydration";
const BENCHMARK_ROLE: &str = "evaluator";

// ---------------------------------------------------------------------------
// Run directory — auto-created per execution with timestamp
// ---------------------------------------------------------------------------

struct RunDir {
    path: PathBuf,
    log: std::fs::File,
}

impl RunDir {
    fn create() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let base = std::env::var("E2E_OUTPUT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/e2e-runs")
            });
        let ts = chrono_stamp();
        let path = base.join(ts);
        fs::create_dir_all(&path)?;
        fs::create_dir_all(path.join("results"))?;
        let log = fs::File::create(path.join("test.log"))?;
        eprintln!("[RUN] output directory: {}", path.display());
        Ok(Self { path, log })
    }

    fn log(&mut self, msg: &str) {
        eprintln!("{msg}");
        let _ = writeln!(self.log, "{msg}");
    }

    fn write_result(
        &self,
        name: &str,
        result: &EvalResult,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let json = serde_json::to_vec_pretty(result)?;
        fs::write(self.path.join("results").join(format!("{name}.json")), json)?;
        Ok(())
    }

    fn write_summary(
        &mut self,
        results: &[EvalResult],
        captured: &[CapturedVariant],
        boot_ms: f64,
        total_ms: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // JSON summary
        let summary = serde_json::to_vec_pretty(results)?;
        fs::write(self.path.join("summary.json"), summary)?;

        // Markdown report
        let mut md = String::new();
        md.push_str("# E2E Evaluation Matrix\n\n");
        md.push_str(&format!("- **Variants captured**: {}\n", captured.len()));
        md.push_str(&format!("- **Evaluations**: {}\n", results.len()));
        md.push_str(&format!("- **Boot time**: {boot_ms:.0}ms\n"));
        md.push_str(&format!("- **Total time**: {total_ms:.0}ms\n\n"));

        md.push_str("| Model | Prompt | Variant | Task | Restart | Reason | Latency |\n");
        md.push_str("|-------|--------|---------|------|---------|--------|---------|\n");
        for r in results {
            let t = match r.task {
                Some(true) => "OK",
                Some(false) => "FAIL",
                None => "ERR",
            };
            let re = match r.restart {
                Some(true) => "OK",
                Some(false) => "FAIL",
                None => "ERR",
            };
            let p = match r.reason {
                Some(true) => "OK",
                Some(false) => "FAIL",
                None => "ERR",
            };
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {:.0}ms |\n",
                r.model, r.prompt, r.variant, t, re, p, r.latency_ms
            ));
        }

        // Aggregates by model x prompt
        md.push_str("\n## Aggregates\n\n");
        md.push_str("| Model | Prompt | Task | Restart | Reason |\n");
        md.push_str("|-------|--------|------|---------|--------|\n");
        let mut seen: Vec<(String, String)> = Vec::new();
        for r in results {
            let key = (r.model.clone(), r.prompt.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let cell: Vec<&EvalResult> = results
                .iter()
                .filter(|x| x.model == r.model && x.prompt == r.prompt)
                .collect();
            let n = cell.iter().filter(|x| x.task.is_some()).count();
            let t = cell.iter().filter(|x| x.task == Some(true)).count();
            let re = cell.iter().filter(|x| x.restart == Some(true)).count();
            let p = cell.iter().filter(|x| x.reason == Some(true)).count();
            md.push_str(&format!(
                "| {} | {} | {}/{} | {}/{} | {}/{} |\n",
                r.model, r.prompt, t, n, re, n, p, n
            ));
        }

        fs::write(self.path.join("report.md"), md)?;

        // Log the path
        self.log(&format!("[RUN] results written to {}", self.path.display()));
        Ok(())
    }
}

fn chrono_stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Simple UTC timestamp without chrono dependency
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;
    // Approximate date from epoch days (good enough for directory names)
    let (year, month, day) = epoch_days_to_date(days);
    format!("{year:04}-{month:02}-{day:02}_{hours:02}{minutes:02}{seconds:02}")
}

fn epoch_days_to_date(days: u64) -> (u64, u64, u64) {
    // Adapted from Howard Hinnant's algorithm
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

struct CapturedVariant {
    run_id: String,
    rendered_content: String,
    rendered_tokens: u32,
    raw_equivalent_tokens: u32,
    compression_ratio: f64,
    causal_density: f64,
    noise_ratio: f64,
    detail_coverage: f64,
    // Kernel domain observability
    resolved_mode: String,
    tier_l0_tokens: u32,
    tier_l1_tokens: u32,
    tier_l2_tokens: u32,
    tier_total_tokens: u32,
    graph_load_ms: f64,
    detail_load_ms: f64,
    bundle_assembly_ms: f64,
    timing_batch_size: u32,
    question: String,
    ground_truth: EvaluationGroundTruth,
}

#[derive(Serialize)]
struct EvalResult {
    model: String,
    prompt: String,
    variant: String,
    task: Option<bool>,
    restart: Option<bool>,
    restart_exact: Option<bool>,
    restart_off_by_one: Option<bool>,
    restart_on_competing: Option<bool>,
    restart_explained: Option<bool>,
    reason: Option<bool>,
    reason_correct: Option<bool>,
    reason_distractor: Option<bool>,
    latency_ms: f64,
    rendered_tokens: u32,
    raw_equivalent_tokens: u32,
    compression_ratio: f64,
    causal_density: f64,
    noise_ratio: f64,
    detail_coverage: f64,
    // Kernel domain observability
    resolved_mode: String,
    tier_l0_tokens: u32,
    tier_l1_tokens: u32,
    tier_l2_tokens: u32,
    tier_total_tokens: u32,
    graph_load_ms: f64,
    detail_load_ms: f64,
    bundle_assembly_ms: f64,
    timing_batch_size: u32,
    llm_prompt_tokens: Option<u32>,
    llm_completion_tokens: Option<u32>,
    llm_reason_source: Option<String>,
    llm_confidence: Option<String>,
    llm_reason_fabricated: Option<bool>,
    agent_response: String,
    judge_raw: Option<String>,
}

fn tier_content_for_eval(rendered: &RenderedContext) -> String {
    if rendered.tiers.is_empty() {
        return rendered.content.clone();
    }
    let mut parts = Vec::new();
    for tier in &rendered.tiers {
        if tier.tier == ResolutionTier::L0Summary as i32
            || tier.tier == ResolutionTier::L1CausalSpine as i32
        {
            parts.push(tier.content.as_str());
        }
    }
    if parts.is_empty() {
        rendered.content.clone()
    } else {
        parts.join("\n\n")
    }
}

// ---------------------------------------------------------------------------
// Precheck — validate all dependencies before booting containers
// ---------------------------------------------------------------------------

struct PrecheckResult {
    pass: bool,
    ok: Vec<String>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

fn precheck_tls(
    matrix: &serde_yaml::Value,
    ok: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let any_tls_agent = matrix["agents"]
        .as_mapping()
        .map(|a| a.values().any(|c| c["tls"].as_bool().unwrap_or(false)))
        .unwrap_or(false);
    if any_tls_agent {
        match (
            matrix["tls"]["cert"].as_str(),
            matrix["tls"]["key"].as_str(),
        ) {
            (Some(c), Some(k)) => {
                ok.push(format!("tls section: cert={c}, key={k}"));
            }
            _ => {
                errors.push(
                    "tls.cert and tls.key required in YAML when any agent has tls: true"
                        .to_string(),
                );
            }
        }
    }
}

fn precheck_agents(
    matrix: &serde_yaml::Value,
    filter_models: &[String],
    valid_providers: &[&str],
    ok: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if let Some(agents) = matrix["agents"].as_mapping() {
        ok.push(format!("{} agents configured", agents.len()));
        for (key, cfg) in agents {
            let name = key.as_str().unwrap_or("?");
            if !filter_models.is_empty() && !filter_models.contains(&name.to_string()) {
                ok.push(format!("agent '{name}': skipped (filtered)"));
                continue;
            }
            if cfg["endpoint"].as_str().unwrap_or("").is_empty() {
                errors.push(format!("agent '{name}': missing endpoint"));
            }
            if cfg["model"].as_str().unwrap_or("").is_empty() {
                errors.push(format!("agent '{name}': missing model"));
            }
            match cfg["provider"].as_str() {
                None => errors.push(format!("agent '{name}': missing provider")),
                Some(p) if !valid_providers.contains(&p) => {
                    errors.push(format!(
                        "agent '{name}': unknown provider '{p}' — expected: {}",
                        valid_providers.join(", ")
                    ));
                }
                Some(_) => {}
            }
            precheck_agent_api_key(name, cfg, ok, errors);
            precheck_agent_tls(name, cfg, matrix, ok, errors);
        }
    } else {
        errors.push("matrix: no agents defined".to_string());
    }
}

fn precheck_agent_api_key(
    name: &str,
    cfg: &serde_yaml::Value,
    ok: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if let Some(env_name) = cfg["api_key_env"].as_str() {
        if std::env::var(env_name).is_err() {
            errors.push(format!("agent '{name}': env var {env_name} not set"));
        } else {
            ok.push(format!("agent '{name}': {env_name} set"));
            if let Some(endpoint) = cfg["endpoint"].as_str() {
                let api_key = std::env::var(env_name).unwrap_or_default();
                let reachable = check_api_endpoint(endpoint, Some(&api_key), None, None);
                if reachable {
                    ok.push(format!("agent '{name}': endpoint reachable"));
                } else {
                    errors.push(format!(
                        "agent '{name}': endpoint unreachable ({endpoint})"
                    ));
                }
            }
        }
    }
}

fn precheck_agent_tls(
    name: &str,
    cfg: &serde_yaml::Value,
    matrix: &serde_yaml::Value,
    ok: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if !cfg["tls"].as_bool().unwrap_or(false) {
        return;
    }
    let cert_path = matrix["tls"]["cert"].as_str();
    let key_path_val = matrix["tls"]["key"].as_str();

    match (cert_path, key_path_val) {
        (None, _) => {
            errors.push(format!("agent '{name}': tls.cert not set in YAML"))
        }
        (_, None) => {
            errors.push(format!("agent '{name}': tls.key not set in YAML"))
        }
        (Some(cert), Some(key)) => {
            if !std::path::Path::new(cert).exists() {
                errors.push(format!(
                    "agent '{name}': TLS cert not found at {cert} (tls.cert in YAML)"
                ));
            } else if !std::path::Path::new(key).exists() {
                errors.push(format!(
                    "agent '{name}': TLS key not found at {key} (tls.key in YAML)"
                ));
            } else {
                ok.push(format!(
                    "agent '{name}': TLS certs present ({cert}, {key})"
                ));
                if let Some(endpoint) = cfg["endpoint"].as_str() {
                    let reachable =
                        check_api_endpoint(endpoint, None, Some(cert), Some(key));
                    if reachable {
                        ok.push(format!(
                            "agent '{name}': TLS endpoint reachable"
                        ));
                    } else {
                        errors.push(format!(
                            "agent '{name}': TLS endpoint unreachable ({endpoint})"
                        ));
                    }
                }
            }
        }
    }
}

fn precheck_judges(
    matrix: &serde_yaml::Value,
    filter_judges: &[String],
    valid_providers: &[&str],
    ok: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if let Some(judges) = matrix["judges"].as_mapping() {
        ok.push(format!("{} judges configured", judges.len()));
        for (key, cfg) in judges {
            let name = key.as_str().unwrap_or("?");
            if !filter_judges.is_empty() && !filter_judges.contains(&name.to_string()) {
                ok.push(format!("judge '{name}': skipped (filtered)"));
                continue;
            }
            if cfg["endpoint"].as_str().unwrap_or("").is_empty() {
                errors.push(format!("judge '{name}': missing endpoint"));
            }
            if cfg["model"].as_str().unwrap_or("").is_empty() {
                errors.push(format!("judge '{name}': missing model"));
            }
            match cfg["provider"].as_str() {
                None => errors.push(format!("judge '{name}': missing provider")),
                Some(p) if !valid_providers.contains(&p) => {
                    errors.push(format!("judge '{name}': unknown provider '{p}'"));
                }
                Some(_) => {}
            }
            if let Some(env_name) = cfg["api_key_env"].as_str() {
                if std::env::var(env_name).is_err() {
                    errors.push(format!("judge '{name}': env var {env_name} not set"));
                } else {
                    ok.push(format!("judge '{name}': {env_name} set"));
                    if let Some(endpoint) = cfg["endpoint"].as_str() {
                        let api_key = std::env::var(env_name).unwrap_or_default();
                        let reachable =
                            check_api_endpoint(endpoint, Some(&api_key), None, None);
                        if reachable {
                            ok.push(format!("judge '{name}': endpoint reachable"));
                        } else {
                            errors.push(format!(
                                "judge '{name}': endpoint unreachable ({endpoint})"
                            ));
                        }
                    }
                }
            }
        }
    } else {
        errors.push("matrix: no judges defined".to_string());
    }
}

fn precheck_prompts(
    matrix: &serde_yaml::Value,
    resources: &str,
    ok: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if let Some(prompts) = matrix["prompts"].as_mapping() {
        for (key, val) in prompts {
            let name = key.as_str().unwrap_or("?");
            if let Some(path) = val.as_str() {
                let full_path = format!("{resources}/{path}");
                if std::path::Path::new(&full_path).exists() {
                    ok.push(format!("prompt '{name}': {path}"));
                } else {
                    errors.push(format!(
                        "prompt '{name}': file not found at {full_path}"
                    ));
                }
            }
            // null means compiled-in default — always ok
        }
    }
}

fn precheck_container_runtime(ok: &mut Vec<String>, errors: &mut Vec<String>) {
    let has_docker = std::process::Command::new("docker")
        .arg("info")
        .output()
        .is_ok_and(|o| o.status.success());
    let has_podman = std::process::Command::new("podman")
        .arg("info")
        .output()
        .is_ok_and(|o| o.status.success());
    if has_docker {
        ok.push("container runtime: docker".to_string());
    } else if has_podman {
        ok.push("container runtime: podman".to_string());
    } else {
        errors.push("no container runtime: install docker or podman".to_string());
    }
}

fn precheck_filters(warnings: &mut Vec<String>) {
    for filter in &[
        "FILTER_MODELS",
        "FILTER_PROMPTS",
        "FILTER_SCALES",
        "FILTER_NOISE",
        "FILTER_JUDGES",
    ] {
        if let Ok(val) = std::env::var(filter) {
            warnings.push(format!("{filter}={val} (subset mode)"));
        }
    }
}

fn precheck(resources: &str, matrix_path: &str) -> PrecheckResult {
    let mut ok = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    let filter_models = env_filter("FILTER_MODELS");
    let filter_judges = env_filter("FILTER_JUDGES");

    // 1. Matrix YAML
    let matrix_path = matrix_path.to_string();
    match std::fs::read_to_string(&matrix_path) {
        Ok(content) => match serde_yaml::from_str::<serde_yaml::Value>(&content) {
            Ok(matrix) => {
                ok.push(format!("evaluation-matrix.yaml loaded ({matrix_path})"));
                let valid_providers = ["openai", "openai-new", "anthropic"];

                precheck_tls(&matrix, &mut ok, &mut errors);
                precheck_agents(&matrix, &filter_models, &valid_providers, &mut ok, &mut errors);
                precheck_judges(&matrix, &filter_judges, &valid_providers, &mut ok, &mut errors);
                precheck_prompts(&matrix, resources, &mut ok, &mut errors);
            }
            Err(e) => errors.push(format!("evaluation-matrix.yaml parse error: {e}")),
        },
        Err(e) => errors.push(format!("evaluation-matrix.yaml not found: {e}")),
    }

    // 3. Container runtime
    precheck_container_runtime(&mut ok, &mut errors);

    // 4. Filters (informational)
    precheck_filters(&mut warnings);

    PrecheckResult {
        pass: errors.is_empty(),
        ok,
        warnings,
        errors,
    }
}

// ---------------------------------------------------------------------------
// Phase 1 helpers: ground truth + kernel observability extraction
// ---------------------------------------------------------------------------

fn build_ground_truth_for_seed(
    seed: &GeneratedSeed,
    domain_name: &str,
) -> (String, EvaluationGroundTruth) {
    let chain_nodes: Vec<&GeneratedNode> = seed
        .nodes
        .iter()
        .filter(|n| !n.node_id.contains("noise") && !n.node_id.contains("distractor"))
        .collect();

    let question = format!(
        "Given this rehydrated context from a {domain_name} graph:\n\
         1. What is the deepest failure point in the causal chain?\n\
         2. Which node should the system restart from to recover?\n\
         3. What rationale connects the nodes in the causal chain?"
    );

    let failure_desc = if let Some(leaf) = chain_nodes.last() {
        format!(
            "{} ({}) \u{2014} {}. Causal chain: {} \u{2192} {}",
            leaf.title,
            leaf.node_id,
            leaf.summary,
            seed.root.title,
            chain_nodes
                .iter()
                .map(|n| n.title.as_str())
                .collect::<Vec<_>>()
                .join(" \u{2192} ")
        )
    } else {
        format!(
            "{} \u{2014} {} (single node, no chain)",
            seed.root.node_kind, seed.root.summary
        )
    };

    let restart_desc = if chain_nodes.len() >= 2 {
        let predecessor = &chain_nodes[chain_nodes.len() - 2];
        format!(
            "{} ({}) \u{2014} causal predecessor of the failure leaf",
            predecessor.title, predecessor.node_id
        )
    } else if let Some(first) = chain_nodes.first() {
        format!(
            "{} ({}) \u{2014} first node after root",
            first.title, first.node_id
        )
    } else {
        format!("{} (root, only node)", seed.root.node_kind)
    };

    let chain_rationales: Vec<String> = seed
        .relations
        .iter()
        .filter(|r| !r.source_node_id.contains("noise"))
        .filter_map(|r| r.rationale.clone())
        .collect();
    let reason = if chain_rationales.is_empty() {
        None
    } else {
        Some(chain_rationales.join("; "))
    };

    let distractor_rationales: Vec<String> = seed
        .relations
        .iter()
        .filter(|r| {
            r.source_node_id.contains("noise") || r.target_node_id.contains("noise")
        })
        .filter_map(|r| r.rationale.clone())
        .collect();
    let distractor = if distractor_rationales.is_empty() {
        None
    } else {
        Some(distractor_rationales.join("; "))
    };

    let ground_truth = EvaluationGroundTruth {
        expected_failure_point: Some(failure_desc),
        expected_restart_node: Some(restart_desc),
        expected_reason: reason,
        distractor_rationale: distractor,
        domain_context: Some(domain_name.to_string()),
    };

    (question, ground_truth)
}

struct KernelObservability {
    resolved_mode: String,
    tier_l0_tokens: u32,
    tier_l1_tokens: u32,
    tier_l2_tokens: u32,
    tier_total_tokens: u32,
    graph_load_ms: f64,
    detail_load_ms: f64,
    bundle_assembly_ms: f64,
    timing_batch_size: u32,
}

fn extract_kernel_observability(
    rendered: &RenderedContext,
    graph_load_ms: f64,
    detail_load_ms: f64,
    bundle_assembly_ms: f64,
    timing_batch_size: u32,
) -> KernelObservability {
    let resolved_mode = match RehydrationMode::try_from(rendered.resolved_mode) {
        Ok(RehydrationMode::ResumeFocused) => "resume_focused",
        Ok(RehydrationMode::ReasonPreserving) => "reason_preserving",
        Ok(RehydrationMode::TemporalDelta) => "temporal_delta",
        Ok(RehydrationMode::GlobalSummary) => "global_summary",
        _ => "auto",
    }
    .to_string();
    let tier_l0_tokens = rendered
        .tiers
        .iter()
        .find(|t| t.tier == ResolutionTier::L0Summary as i32)
        .map(|t| t.token_count)
        .unwrap_or(0);
    let tier_l1_tokens = rendered
        .tiers
        .iter()
        .find(|t| t.tier == ResolutionTier::L1CausalSpine as i32)
        .map(|t| t.token_count)
        .unwrap_or(0);
    let tier_l2_tokens = rendered
        .tiers
        .iter()
        .find(|t| t.tier == ResolutionTier::L2EvidencePack as i32)
        .map(|t| t.token_count)
        .unwrap_or(0);

    KernelObservability {
        resolved_mode,
        tier_l0_tokens,
        tier_l1_tokens,
        tier_l2_tokens,
        tier_total_tokens: tier_l0_tokens + tier_l1_tokens + tier_l2_tokens,
        graph_load_ms,
        detail_load_ms,
        bundle_assembly_ms,
        timing_batch_size,
    }
}

// ---------------------------------------------------------------------------
// Phase 2 helpers: evaluation result construction
// ---------------------------------------------------------------------------

fn build_eval_result_from_ctx(ctx: &CapturedVariant, model: String, prompt_name: &str) -> EvalResult {
    EvalResult {
        model,
        prompt: prompt_name.to_string(),
        variant: ctx.run_id.clone(),
        task: None,
        restart: None,
        restart_exact: None,
        restart_off_by_one: None,
        restart_on_competing: None,
        restart_explained: None,
        reason: None,
        reason_correct: None,
        reason_distractor: None,
        latency_ms: 0.0,
        rendered_tokens: ctx.rendered_tokens,
        raw_equivalent_tokens: ctx.raw_equivalent_tokens,
        compression_ratio: ctx.compression_ratio,
        causal_density: ctx.causal_density,
        noise_ratio: ctx.noise_ratio,
        detail_coverage: ctx.detail_coverage,
        resolved_mode: ctx.resolved_mode.clone(),
        tier_l0_tokens: ctx.tier_l0_tokens,
        tier_l1_tokens: ctx.tier_l1_tokens,
        tier_l2_tokens: ctx.tier_l2_tokens,
        tier_total_tokens: ctx.tier_total_tokens,
        graph_load_ms: ctx.graph_load_ms,
        detail_load_ms: ctx.detail_load_ms,
        bundle_assembly_ms: ctx.bundle_assembly_ms,
        timing_batch_size: ctx.timing_batch_size,
        llm_prompt_tokens: None,
        llm_completion_tokens: None,
        llm_reason_source: None,
        llm_confidence: None,
        llm_reason_fabricated: None,
        agent_response: String::new(),
        judge_raw: None,
    }
}

// ---------------------------------------------------------------------------
// Phase 3 helpers: report generation
// ---------------------------------------------------------------------------

fn format_verdict(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "OK",
        Some(false) => "FAIL",
        None => "ERR",
    }
}

fn write_report(
    run: &mut RunDir,
    results: &[EvalResult],
    captured: &[CapturedVariant],
    boot_ms: f64,
    total_ms: f64,
) {
    run.log(&format!(
        "\n\n{}",
        "=".repeat(120)
    ));
    run.log(&format!(
        "EVALUATION MATRIX \u{2014} {} variants captured, {} evals, boot {boot_ms:.0}ms, total {total_ms:.0}ms",
        captured.len(),
        results.len()
    ));
    run.log(&format!("{}\n", "=".repeat(120)));

    for r in results {
        let ctx = captured.iter().find(|c| c.run_id == r.variant);
        let question = ctx.map(|c| c.question.as_str()).unwrap_or("?");
        let expected = ctx
            .and_then(|c| c.ground_truth.expected_reason.as_deref())
            .unwrap_or("none");
        let tokens = ctx.map(|c| c.rendered_tokens).unwrap_or(0);

        let t = format_verdict(r.task);
        let re = format_verdict(r.restart);
        let p = format_verdict(r.reason);

        run.log(&format!(
            "\u{250c}\u{2500}\u{2500} {:<14} \u{00d7} {:<14} / {} ({tokens} tok) \u{2500}\u{2500}",
            r.model, r.prompt, r.variant
        ));
        run.log(&format!("\u{2502} 1. Question:        {question}"));
        run.log(&format!("\u{2502} 2. Expected reason: {expected}"));
        let resp = r.agent_response.replace('\n', " ");
        run.log(&format!("\u{2502} 3. Agent response:  {resp}"));
        run.log(&format!(
            "\u{2502} 4. Judge response:  {}",
            r.judge_raw.as_deref().unwrap_or("(none)")
        ));
        run.log(&format!(
            "\u{2502} 5. Result:          Task={t}  Restart={re}  Reason={p}  ({:.0}ms)",
            r.latency_ms
        ));
        run.log("\u{2514}\u{2500}\u{2500}\n");
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn judge_prompt_evaluation_across_all_use_cases() -> Result<(), Box<dyn Error + Send + Sync>>
{
    let resources = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/rehydration-testkit/resources"
    );
    let matrix_path = std::env::var("EVAL_MATRIX_PATH")
        .unwrap_or_else(|_| format!("{resources}/evaluation-matrix.yaml"));

    // ── Precheck: validate everything before booting containers ──
    let precheck = precheck(resources, &matrix_path);
    if !precheck.pass {
        eprintln!("\n{}", "=".repeat(70));
        eprintln!("  PRECHECK FAILED — fix the issues below before running");
        eprintln!("{}\n", "=".repeat(70));
        for msg in &precheck.errors {
            eprintln!("  \u{2718} {msg}");
        }
        eprintln!();
        for msg in &precheck.warnings {
            eprintln!("  \u{26a0} {msg}");
        }
        eprintln!();
        panic!("precheck failed: {} error(s)", precheck.errors.len());
    }
    for msg in &precheck.warnings {
        eprintln!("  \u{26a0} {msg}");
    }
    for msg in &precheck.ok {
        eprintln!("  \u{2714} {msg}");
    }
    eprintln!();

    let mut run = RunDir::create()?;
    let boot_start = Instant::now();

    let matrix: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&matrix_path)?)?;
    run.log(&format!("[CONFIG] matrix={matrix_path}"));

    let filter_models = env_filter("FILTER_MODELS");
    let filter_prompts = env_filter("FILTER_PROMPTS");
    let filter_scales = env_filter("FILTER_SCALES");
    let filter_noise = env_filter("FILTER_NOISE");

    // ── Phase 0: Judge calibration ──
    // Send known-good and known-bad synthetic responses to each judge
    // to verify the judge model + prompt combination is sane before
    // committing to the full benchmark. Aborts early on miscalibration.

    let judges = matrix["judges"].as_mapping().expect("judges mapping");
    let prompts_map = matrix["prompts"].as_mapping().expect("prompts mapping");
    let filter_judges = env_filter("FILTER_JUDGES");

    // Calibrate with the default prompt and first non-filtered judge
    let default_prompt = PromptConfig::load(None)?;
    for (judge_key, judge_cfg) in judges {
        let judge_name = judge_key.as_str().expect("judge key");
        if !filter_judges.is_empty() && !filter_judges.contains(&judge_name.to_string()) {
            continue;
        }

        // Build a judge-only config (no agent needed for calibration)
        let cal_config = LlmEvaluatorConfig {
            endpoint: yaml_str(judge_cfg, "endpoint", "judge"),
            model: yaml_str(judge_cfg, "model", "judge"),
            provider: parse_provider(&yaml_str(judge_cfg, "provider", "judge")),
            api_key: judge_cfg["api_key_env"]
                .as_str()
                .and_then(|e| std::env::var(e).ok()),
            max_tokens: 200,
            temperature: 0.0,
            tls_cert_path: None,
            tls_key_path: None,
            tls_insecure: false,
            judge_endpoint: Some(yaml_str(judge_cfg, "endpoint", "judge")),
            judge_model: Some(yaml_str(judge_cfg, "model", "judge")),
            judge_provider: Some(parse_provider(&yaml_str(judge_cfg, "provider", "judge"))),
            judge_api_key: judge_cfg["api_key_env"]
                .as_str()
                .and_then(|e| std::env::var(e).ok()),
        };

        run.log(&format!(
            "\n[CALIBRATION] Testing judge '{judge_name}' with known-good/known-bad cases..."
        ));
        let cases = calibrate_judge(&default_prompt, &cal_config).await?;
        let mut cal_failed = false;
        for case in &cases {
            let icon = if case.passed { "\u{2714}" } else { "\u{2718}" };
            run.log(&format!(
                "  {icon} {}: expected={}, got={}",
                case.name, case.expected, case.got
            ));
            if !case.passed {
                cal_failed = true;
            }
        }
        if cal_failed {
            run.log(&format!("[CALIBRATION] FAILED for judge '{judge_name}' — aborting to avoid wasting eval budget"));
            panic!(
                "judge calibration failed for '{judge_name}': judge is miscalibrated, fix prompt or switch model"
            );
        }
        run.log(&format!(
            "[CALIBRATION] Judge '{judge_name}' passed ({} cases)\n",
            cases.len()
        ));
    }

    // ── Phase 1: Boot + capture ──

    type ScaleEntry = (&'static str, fn(Domain) -> GraphSeedConfig);
    let all_scales: Vec<ScaleEntry> = vec![
        ("micro", |d| GraphSeedConfig::micro(d)),
        ("meso", |d| GraphSeedConfig::meso(d)),
        ("stress", |d| GraphSeedConfig::stress(d)),
    ];
    let yaml_scales: Vec<String> = matrix["scales"]
        .as_mapping()
        .map(|m| {
            m.keys()
                .filter_map(|k| k.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let scales: Vec<ScaleEntry> = if yaml_scales.is_empty() {
        all_scales
    } else {
        all_scales
            .into_iter()
            .filter(|(name, _)| yaml_scales.iter().any(|s| s == name))
            .collect()
    };
    let domains = [
        ("ops", Domain::Operations),
        ("debug", Domain::SoftwareDebugging),
    ];
    let mixes = [
        ("explanatory", RelationMix::Explanatory),
        ("structural", RelationMix::Structural),
        ("mixed", RelationMix::Mixed),
    ];
    // Noise modes are distributed across mixes rather than fully crossed,
    // keeping the total variant count constant at 36 (same as the original
    // binary clean/competing). Each mix gets 2 noise modes:
    //   explanatory → clean + competing (rationale preservation under noise)
    //   structural  → clean + conflicting (resistance to contradictions)
    //   mixed       → clean + restart (restart accuracy under pressure)
    // This ensures every noise mode is tested while the eval budget stays flat.
    let noise_for_mix: &[(&str, &[(&str, NoiseMode)])] = &[
        (
            "explanatory",
            &[
                ("clean", NoiseMode::Structural),
                ("competing", NoiseMode::CompetingCausal),
            ],
        ),
        (
            "structural",
            &[
                ("clean", NoiseMode::Structural),
                ("conflicting", NoiseMode::ConflictingMainPath),
            ],
        ),
        (
            "mixed",
            &[
                ("clean", NoiseMode::Structural),
                ("restart", NoiseMode::CompetingRestartPoint),
            ],
        ),
    ];

    let seeds_per_cell = matrix["seeds_per_cell"].as_u64().unwrap_or(1) as usize;
    let active_scale_names: Vec<&str> = scales.iter().map(|(name, _)| *name).collect();
    run.log(&format!(
        "[CONFIG] seeds_per_cell={seeds_per_cell}, scales={active_scale_names:?}"
    ));

    let mut captured: Vec<CapturedVariant> = Vec::new();
    let mut fixture: Option<TestFixture> = None;

    for &(scale_name, scale_fn) in &scales {
        if !filter_scales.is_empty() && !filter_scales.contains(&scale_name.to_string()) {
            continue;
        }
        for &(domain_name, domain) in &domains {
            for &(mix_name, mix) in &mixes {
                let noises = noise_for_mix
                    .iter()
                    .find(|(m, _)| *m == mix_name)
                    .map(|(_, n)| *n)
                    .unwrap_or(&[("clean", NoiseMode::Structural)]);
                for &(noise_name, noise_mode) in noises {
                    if !filter_noise.is_empty() && !filter_noise.contains(&noise_name.to_string()) {
                        continue;
                    }
                    for seed_idx in 0..seeds_per_cell {
                        let mut config = scale_fn(domain);
                        config.relation_mix = mix;
                        config.noise_mode = noise_mode;
                        config.seed = seed_idx;
                        let variant_id = if seeds_per_cell > 1 {
                            format!(
                                "{scale_name}-{domain_name}-{mix_name}-{noise_name}-s{seed_idx}"
                            )
                        } else {
                            format!("{scale_name}-{domain_name}-{mix_name}-{noise_name}")
                        };
                        config.id_prefix = variant_id.clone();

                        let seed = generate_seed(config.clone());
                        let events = seed_to_projection_events(&seed, SUBJECT_PREFIX, &variant_id)?;

                        let root_id = seed.root.node_id.clone();
                        let focus_id = seed
                            .nodes
                            .first()
                            .map(|n| n.node_id.clone())
                            .unwrap_or_else(|| root_id.clone());

                        match &fixture {
                            None => {
                                run.log(&format!("[INFRA] Booting containers for {variant_id}..."));
                                let f = TestFixture::builder()
                                    .with_neo4j()
                                    .with_valkey()
                                    .with_nats()
                                    .with_projection_runtime()
                                    .with_grpc_server()
                                    .build()
                                    .await?;

                                let client = connect_nats_with_retry(f.nats_url()).await?;
                                for (subject, payload) in &events {
                                    client
                                        .publish(subject.clone(), payload.clone().into())
                                        .await?;
                                }
                                client.flush().await?;

                                wait_for_context(&f, &root_id, &focus_id).await?;
                                fixture = Some(f);
                            }
                            Some(f) => {
                                run.log(&format!("[INFRA] Reseeding {variant_id}..."));
                                let client = connect_nats_with_retry(f.nats_url()).await?;
                                for (subject, payload) in &events {
                                    client
                                        .publish(subject.clone(), payload.clone().into())
                                        .await?;
                                }
                                client.flush().await?;

                                wait_for_context(f, &root_id, &focus_id).await?;
                            }
                        }

                        sleep(Duration::from_millis(500)).await;

                        let mut qc = fixture
                            .as_ref()
                            .expect("fixture should be Some")
                            .query_client();
                        let response = qc
                            .get_context(GetContextRequest {
                                root_node_id: root_id.clone(),
                                role: BENCHMARK_ROLE.to_string(),
                                token_budget: 4096,
                                requested_scopes: vec![],
                                depth: config.chain_length as u32,
                                max_tier: 0,
                                rehydration_mode: 0,
                            })
                            .await?
                            .into_inner();

                        let rendered = response.rendered.ok_or("missing rendered")?;
                        let eval_content = tier_content_for_eval(&rendered);
                        let tokens = rendered.token_count;
                        let quality = rendered
                            .quality
                            .ok_or("missing quality metrics in rendered context")?;

                        // Build ground truth that rewards multi-layer graph reasoning.
                        let (question, ground_truth) =
                            build_ground_truth_for_seed(&seed, domain_name);

                        let chain_kinds: String = seed
                            .nodes
                            .iter()
                            .filter(|n| {
                                !n.node_id.contains("noise") && !n.node_id.contains("distractor")
                            })
                            .map(|n| n.node_kind.as_str())
                            .collect::<Vec<_>>()
                            .join("\u{2192}");
                        run.log(&format!("[CAPTURE] {variant_id}: {tokens} tok (raw={}, compress={:.2}x, causal={:.0}%, noise={:.0}%, detail={:.0}%), chain=[{chain_kinds}], reason={}, distractor={}",
                    quality.raw_equivalent_tokens, quality.compression_ratio,
                    quality.causal_density * 100.0, quality.noise_ratio * 100.0, quality.detail_coverage * 100.0,
                    ground_truth.expected_reason.as_deref().unwrap_or("none"),
                    ground_truth.distractor_rationale.as_deref().unwrap_or("none")));

                        // Kernel domain observability
                        let timing = response.timing.as_ref();
                        let obs = extract_kernel_observability(
                            &rendered,
                            timing.map(|t| t.graph_load_seconds * 1000.0).unwrap_or(0.0),
                            timing.map(|t| t.detail_load_seconds * 1000.0).unwrap_or(0.0),
                            timing.map(|t| t.bundle_assembly_seconds * 1000.0).unwrap_or(0.0),
                            timing.map(|t| t.batch_size).unwrap_or(0),
                        );

                        captured.push(CapturedVariant {
                            run_id: variant_id,
                            rendered_content: eval_content,
                            rendered_tokens: tokens,
                            raw_equivalent_tokens: quality.raw_equivalent_tokens,
                            compression_ratio: quality.compression_ratio,
                            causal_density: quality.causal_density,
                            noise_ratio: quality.noise_ratio,
                            detail_coverage: quality.detail_coverage,
                            resolved_mode: obs.resolved_mode,
                            tier_l0_tokens: obs.tier_l0_tokens,
                            tier_l1_tokens: obs.tier_l1_tokens,
                            tier_l2_tokens: obs.tier_l2_tokens,
                            tier_total_tokens: obs.tier_total_tokens,
                            graph_load_ms: obs.graph_load_ms,
                            detail_load_ms: obs.detail_load_ms,
                            bundle_assembly_ms: obs.bundle_assembly_ms,
                            timing_batch_size: obs.timing_batch_size,
                            question,
                            ground_truth,
                        });
                    } // seed_idx
                }
            }
        }
    }

    let boot_ms = boot_start.elapsed().as_secs_f64() * 1000.0;
    run.log(&format!(
        "[INFRA] {} variants captured in {boot_ms:.0}ms\n",
        captured.len()
    ));

    // ── Phase 2: Evaluate model x prompt x captured variant ──

    let agents = matrix["agents"].as_mapping().expect("agents mapping");
    let prompts = prompts_map;

    let mut results: Vec<EvalResult> = Vec::new();
    let mut eval_count = 0u32;

    for (agent_key, agent_cfg) in agents {
        let agent_name = agent_key.as_str().expect("agent key");
        if !filter_models.is_empty() && !filter_models.contains(&agent_name.to_string()) {
            continue;
        }

        for (judge_key, judge_cfg) in judges {
            let judge_name = judge_key.as_str().expect("judge key");
            if !filter_judges.is_empty() && !filter_judges.contains(&judge_name.to_string()) {
                continue;
            }
            if agent_name == judge_name {
                continue;
            }

            let llm_config = build_llm_config(&matrix, agent_cfg, judge_cfg);

            for (prompt_key, prompt_val) in prompts {
                let prompt_name = prompt_key.as_str().expect("prompt key");
                if !filter_prompts.is_empty() && !filter_prompts.contains(&prompt_name.to_string())
                {
                    continue;
                }

                let prompt_file = prompt_val.as_str().map(|p| format!("{resources}/{p}"));
                let prompts_cfg = PromptConfig::load(prompt_file.as_deref())?;

                run.log(&format!("\n\u{2588}\u{2588}\u{2588}\u{2588} {agent_name}\u{2192}{judge_name}/{prompt_name} \u{2588}\u{2588}\u{2588}\u{2588}"));

                for ctx in &captured {
                    eval_count += 1;
                    let eval = evaluate_with_config(
                        &prompts_cfg,
                        &llm_config,
                        &ctx.rendered_content,
                        &ctx.question,
                        &ctx.ground_truth,
                    )
                    .await;

                    let cell_id = format!("{agent_name}\u{2192}{judge_name}");
                    let result = match eval {
                        Ok(e) => {
                            let t = if e.llm_task_success { "OK" } else { "FAIL" };
                            let r = if e.llm_restart_accuracy { "OK" } else { "FAIL" };
                            let rx = if e.llm_restart_exact { "OK" } else { "FAIL" };
                            let ro = if e.llm_restart_off_by_one {
                                "OK"
                            } else {
                                "FAIL"
                            };
                            let rcb = if e.llm_restart_on_competing {
                                "OK"
                            } else {
                                "FAIL"
                            };
                            let re = if e.llm_restart_explained {
                                "OK"
                            } else {
                                "FAIL"
                            };
                            let rc = if e.llm_reason_correct { "OK" } else { "FAIL" };
                            let rd = if e.llm_reason_distractor {
                                "LEAK"
                            } else {
                                "clean"
                            };
                            let resp_short: String = e
                                .llm_response
                                .replace('\n', " ")
                                .chars()
                                .take(120)
                                .collect();
                            run.log(&format!("  [{cell_id}/{prompt_name}] {}: T={t} R={r}(x={rx} o1={ro} cb={rcb} ex={re}) Rc={rc} Rd={rd}  agent=\"{resp_short}...\"  judge={}",
                                ctx.run_id, e.llm_judge_raw.as_deref().unwrap_or("?")));
                            EvalResult {
                                model: cell_id,
                                prompt: prompt_name.to_string(),
                                variant: ctx.run_id.clone(),
                                task: Some(e.llm_task_success),
                                restart: Some(e.llm_restart_accuracy),
                                restart_exact: Some(e.llm_restart_exact),
                                restart_off_by_one: Some(e.llm_restart_off_by_one),
                                restart_on_competing: Some(e.llm_restart_on_competing),
                                restart_explained: Some(e.llm_restart_explained),
                                reason: Some(e.llm_reason_preserved),
                                reason_correct: Some(e.llm_reason_correct),
                                reason_distractor: Some(e.llm_reason_distractor),
                                latency_ms: e.llm_latency_ms,
                                rendered_tokens: ctx.rendered_tokens,
                                raw_equivalent_tokens: ctx.raw_equivalent_tokens,
                                compression_ratio: ctx.compression_ratio,
                                causal_density: ctx.causal_density,
                                noise_ratio: ctx.noise_ratio,
                                detail_coverage: ctx.detail_coverage,
                                resolved_mode: ctx.resolved_mode.clone(),
                                tier_l0_tokens: ctx.tier_l0_tokens,
                                tier_l1_tokens: ctx.tier_l1_tokens,
                                tier_l2_tokens: ctx.tier_l2_tokens,
                                tier_total_tokens: ctx.tier_total_tokens,
                                graph_load_ms: ctx.graph_load_ms,
                                detail_load_ms: ctx.detail_load_ms,
                                bundle_assembly_ms: ctx.bundle_assembly_ms,
                                timing_batch_size: ctx.timing_batch_size,
                                llm_prompt_tokens: Some(e.llm_prompt_tokens),
                                llm_completion_tokens: Some(e.llm_completion_tokens),
                                llm_reason_source: Some(e.llm_reason_source),
                                llm_confidence: Some(e.llm_confidence),
                                llm_reason_fabricated: Some(e.llm_reason_fabricated),
                                agent_response: e.llm_response,
                                judge_raw: e.llm_judge_raw,
                            }
                        }
                        Err(err) => {
                            run.log(&format!(
                                "  [{cell_id}/{prompt_name}] {}: ERROR {err}",
                                ctx.run_id
                            ));
                            build_eval_result_from_ctx(ctx, cell_id, prompt_name)
                        }
                    };

                    let result_name = format!(
                        "{eval_count:04}_{agent_name}_{judge_name}_{prompt_name}_{}",
                        ctx.run_id
                    );
                    run.write_result(&result_name, &result)?;
                    results.push(result);
                }
            }
        }
    }

    let total_ms = boot_start.elapsed().as_secs_f64() * 1000.0;

    // ── Phase 3: Report ──

    write_report(&mut run, &results, &captured, boot_ms, total_ms);
    print_summary(&mut run, &results);
    run.write_summary(&results, &captured, boot_ms, total_ms)?;

    if let Some(f) = fixture {
        f.shutdown().await?;
    }
    Ok(())
}

fn print_summary(run: &mut RunDir, results: &[EvalResult]) {
    run.log(&format!("\u{2554}{}\u{2557}", "\u{2550}".repeat(78)));
    run.log(&format!("\u{2551}  SUMMARY{}\u{2551}", " ".repeat(70)));
    run.log(&format!("\u{2560}{}\u{2563}", "\u{2550}".repeat(78)));
    run.log(&format!(
        "\u{2551}  {:<16} {:<16} {:>8} {:>8} {:>8} {:>8} \u{2551}",
        "Model", "Prompt", "Task", "Restart", "Reason", "LLM ms"
    ));
    run.log(&format!("\u{2560}{}\u{2563}", "\u{2550}".repeat(78)));

    let mut seen: Vec<(String, String)> = Vec::new();
    for r in results {
        let key = (r.model.clone(), r.prompt.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);

        let cell: Vec<&EvalResult> = results
            .iter()
            .filter(|x| x.model == r.model && x.prompt == r.prompt)
            .collect();
        let n = cell.iter().filter(|x| x.task.is_some()).count();
        let t = cell.iter().filter(|x| x.task == Some(true)).count();
        let re = cell.iter().filter(|x| x.restart == Some(true)).count();
        let p = cell.iter().filter(|x| x.reason == Some(true)).count();
        let ms: f64 = cell.iter().map(|x| x.latency_ms).sum();

        run.log(&format!(
            "\u{2551}  {:<16} {:<16} {:>3}/{:<4} {:>3}/{:<4} {:>3}/{:<4} {:>6.0} \u{2551}",
            r.model, r.prompt, t, n, re, n, p, n, ms
        ));
    }
    run.log(&format!("\u{255a}{}\u{255d}", "\u{2550}".repeat(78)));

    run.log("\nBy scale:");
    for scale in &["micro", "meso", "stress"] {
        let sr: Vec<&EvalResult> = results
            .iter()
            .filter(|r| r.variant.starts_with(scale))
            .collect();
        if sr.is_empty() {
            continue;
        }
        let n = sr.iter().filter(|x| x.task.is_some()).count();
        let t = sr.iter().filter(|x| x.task == Some(true)).count();
        let p = sr.iter().filter(|x| x.reason == Some(true)).count();
        run.log(&format!("  {scale:<8}: Task {t}/{n}  Reason {p}/{n}"));
    }

    run.log("\nBy relation mix:");
    for mix in &["explanatory", "structural", "mixed"] {
        let mr: Vec<&EvalResult> = results.iter().filter(|r| r.variant.contains(mix)).collect();
        if mr.is_empty() {
            continue;
        }
        let n = mr.iter().filter(|x| x.task.is_some()).count();
        let t = mr.iter().filter(|x| x.task == Some(true)).count();
        let p = mr.iter().filter(|x| x.reason == Some(true)).count();
        run.log(&format!("  {mix:<14}: Task {t}/{n}  Reason {p}/{n}"));
    }
}

/// Checks that an LLM endpoint is reachable with a real HTTP request.
/// For OpenAI-compatible: GET /v1/models. For Anthropic: GET /v1/models (returns 404 but proves connectivity).
fn check_api_endpoint(
    endpoint: &str,
    api_key: Option<&str>,
    tls_cert: Option<&str>,
    tls_key: Option<&str>,
) -> bool {
    let probe_url = endpoint
        .replace("/v1/chat/completions", "/v1/models")
        .replace("/v1/messages", "/v1/models");

    let mut args = vec![
        "-s",
        "--max-time",
        "10",
        "-o",
        "/dev/null",
        "-w",
        "%{http_code}",
    ];
    if tls_cert.is_some() {
        args.push("-k");
    }
    if let Some(cert) = tls_cert {
        args.push("--cert");
        args.push(cert);
    }
    if let Some(key) = tls_key {
        args.push("--key");
        args.push(key);
    }

    let auth_header;
    let anthropic_header;
    let anthropic_version;
    if let Some(key) = api_key {
        if endpoint.contains("anthropic.com") {
            anthropic_header = format!("x-api-key: {key}");
            anthropic_version = "anthropic-version: 2023-06-01".to_string();
            args.extend(["-H", &anthropic_header, "-H", &anthropic_version]);
        } else {
            auth_header = format!("Authorization: Bearer {key}");
            args.extend(["-H", &auth_header]);
        }
    }
    args.push(&probe_url);

    match std::process::Command::new("curl").args(&args).output() {
        Ok(out) => {
            let code = String::from_utf8_lossy(&out.stdout);
            // Any HTTP response means the endpoint is reachable.
            // 200 = OK, 401 = auth issue but reachable, 404 = wrong path but reachable.
            // Only 000 (connection refused/timeout) is a real failure.
            code.trim() != "000"
        }
        Err(_) => false,
    }
}

fn env_filter(key: &str) -> Vec<String> {
    std::env::var(key)
        .map(|s| s.split(',').map(str::trim).map(String::from).collect())
        .unwrap_or_default()
}

fn parse_provider(s: &str) -> LlmProvider {
    match s {
        "openai" => LlmProvider::OpenAI,
        "openai-new" => LlmProvider::OpenAINew,
        "anthropic" => LlmProvider::Anthropic,
        other => {
            panic!("unknown provider '{other}' in YAML — expected: openai, openai-new, anthropic")
        }
    }
}

fn yaml_str(cfg: &serde_yaml::Value, field: &str, context: &str) -> String {
    cfg[field]
        .as_str()
        .unwrap_or_else(|| panic!("{context}: missing required field '{field}' in YAML"))
        .to_string()
}

fn build_llm_config(
    matrix: &serde_yaml::Value,
    agent_cfg: &serde_yaml::Value,
    judge_cfg: &serde_yaml::Value,
) -> LlmEvaluatorConfig {
    let tls = agent_cfg["tls"].as_bool().unwrap_or(false);
    let tls_section = &matrix["tls"];
    let tls_cert = if tls {
        Some(yaml_str(tls_section, "cert", "tls"))
    } else {
        None
    };
    let tls_key = if tls {
        Some(yaml_str(tls_section, "key", "tls"))
    } else {
        None
    };

    LlmEvaluatorConfig {
        endpoint: yaml_str(agent_cfg, "endpoint", "agent"),
        model: yaml_str(agent_cfg, "model", "agent"),
        provider: parse_provider(&yaml_str(agent_cfg, "provider", "agent")),
        api_key: agent_cfg["api_key_env"]
            .as_str()
            .and_then(|e| std::env::var(e).ok()),
        max_tokens: 200,
        temperature: 0.0,
        tls_cert_path: tls_cert,
        tls_key_path: tls_key,
        tls_insecure: tls,
        judge_endpoint: Some(yaml_str(judge_cfg, "endpoint", "judge")),
        judge_model: Some(yaml_str(judge_cfg, "model", "judge")),
        judge_provider: Some(parse_provider(&yaml_str(judge_cfg, "provider", "judge"))),
        judge_api_key: judge_cfg["api_key_env"]
            .as_str()
            .and_then(|e| std::env::var(e).ok()),
    }
}

async fn wait_for_context(
    fixture: &TestFixture,
    root_node_id: &str,
    _focus_node_id: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut qc = fixture.query_client();
    for _ in 0..40 {
        if let Ok(resp) = qc
            .get_context(GetContextRequest {
                root_node_id: root_node_id.to_string(),
                role: BENCHMARK_ROLE.to_string(),
                token_budget: 1200,
                requested_scopes: vec!["implementation".to_string()],
                depth: 0,
                max_tier: 0,
                rehydration_mode: 0,
            })
            .await
        {
            let resp = resp.into_inner();
            if let Some(bundle) = resp.bundle
                && bundle.root_node_id == root_node_id
                && bundle
                    .bundles
                    .first()
                    .is_some_and(|b| !b.neighbor_nodes.is_empty())
            {
                return Ok(());
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err("context did not become ready".into())
}
