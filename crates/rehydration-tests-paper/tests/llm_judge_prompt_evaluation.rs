#![cfg(feature = "container-tests")]
#![allow(deprecated)]

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rehydration_proto::v1beta1::{
    BundleRenderFormat, GetContextRequest, Phase, RenderedContext, ResolutionTier,
};
use rehydration_testkit::{
    Domain, EvaluationGroundTruth, GraphSeedConfig, LlmEvaluatorConfig, LlmProvider,
    NoiseMode, PromptConfig, RelationMix, evaluate_with_config, generate_seed,
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
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../artifacts/e2e-runs")
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

    fn write_result(&self, name: &str, result: &EvalResult) -> Result<(), Box<dyn Error + Send + Sync>> {
        let json = serde_json::to_vec_pretty(result)?;
        fs::write(self.path.join("results").join(format!("{name}.json")), json)?;
        Ok(())
    }

    fn write_summary(&mut self, results: &[EvalResult], captured: &[CapturedVariant], boot_ms: f64, total_ms: f64) -> Result<(), Box<dyn Error + Send + Sync>> {
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
            let t = match r.task { Some(true) => "OK", Some(false) => "FAIL", None => "ERR" };
            let re = match r.restart { Some(true) => "OK", Some(false) => "FAIL", None => "ERR" };
            let p = match r.reason { Some(true) => "OK", Some(false) => "FAIL", None => "ERR" };
            md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {:.0}ms |\n",
                r.model, r.prompt, r.variant, t, re, p, r.latency_ms));
        }

        // Aggregates by model x prompt
        md.push_str("\n## Aggregates\n\n");
        md.push_str("| Model | Prompt | Task | Restart | Reason |\n");
        md.push_str("|-------|--------|------|---------|--------|\n");
        let mut seen: Vec<(String, String)> = Vec::new();
        for r in results {
            let key = (r.model.clone(), r.prompt.clone());
            if seen.contains(&key) { continue; }
            seen.push(key);
            let cell: Vec<&EvalResult> = results.iter()
                .filter(|x| x.model == r.model && x.prompt == r.prompt)
                .collect();
            let n = cell.iter().filter(|x| x.task.is_some()).count();
            let t = cell.iter().filter(|x| x.task == Some(true)).count();
            let re = cell.iter().filter(|x| x.restart == Some(true)).count();
            let p = cell.iter().filter(|x| x.reason == Some(true)).count();
            md.push_str(&format!("| {} | {} | {}/{} | {}/{} | {}/{} |\n",
                r.model, r.prompt, t, n, re, n, p, n));
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
    reason: Option<bool>,
    latency_ms: f64,
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
    if parts.is_empty() { rendered.content.clone() } else { parts.join("\n\n") }
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

fn precheck(resources: &str) -> PrecheckResult {
    let mut ok = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // 1. Matrix YAML
    let matrix_path = format!("{resources}/evaluation-matrix.yaml");
    match std::fs::read_to_string(&matrix_path) {
        Ok(content) => match serde_yaml::from_str::<serde_yaml::Value>(&content) {
            Ok(matrix) => {
                ok.push(format!("evaluation-matrix.yaml loaded ({matrix_path})"));
                let valid_providers = ["openai", "openai-new", "anthropic"];

                // Validate TLS section
                let any_tls_agent = matrix["agents"].as_mapping()
                    .map(|a| a.values().any(|c| c["tls"].as_bool().unwrap_or(false)))
                    .unwrap_or(false);
                if any_tls_agent {
                    match (matrix["tls"]["cert"].as_str(), matrix["tls"]["key"].as_str()) {
                        (Some(c), Some(k)) => {
                            ok.push(format!("tls section: cert={c}, key={k}"));
                        }
                        _ => {
                            errors.push("tls.cert and tls.key required in YAML when any agent has tls: true".to_string());
                        }
                    }
                }

                // Validate agents
                if let Some(agents) = matrix["agents"].as_mapping() {
                    ok.push(format!("{} agents configured", agents.len()));
                    for (key, cfg) in agents {
                        let name = key.as_str().unwrap_or("?");
                        if cfg["endpoint"].as_str().unwrap_or("").is_empty() {
                            errors.push(format!("agent '{name}': missing endpoint"));
                        }
                        if cfg["model"].as_str().unwrap_or("").is_empty() {
                            errors.push(format!("agent '{name}': missing model"));
                        }
                        match cfg["provider"].as_str() {
                            None => errors.push(format!("agent '{name}': missing provider")),
                            Some(p) if !valid_providers.contains(&p) => {
                                errors.push(format!("agent '{name}': unknown provider '{p}' — expected: {}", valid_providers.join(", ")));
                            }
                            Some(_) => {}
                        }
                        // Check API key env var if specified
                        if let Some(env_name) = cfg["api_key_env"].as_str() {
                            if std::env::var(env_name).is_err() {
                                errors.push(format!("agent '{name}': env var {env_name} not set"));
                            } else {
                                ok.push(format!("agent '{name}': {env_name} set"));
                                // Connectivity check
                                if let Some(endpoint) = cfg["endpoint"].as_str() {
                                    let api_key = std::env::var(env_name).unwrap_or_default();
                                    let reachable = check_api_endpoint(endpoint, Some(&api_key), None, None);
                                    if reachable {
                                        ok.push(format!("agent '{name}': endpoint reachable"));
                                    } else {
                                        errors.push(format!("agent '{name}': endpoint unreachable ({endpoint})"));
                                    }
                                }
                            }
                        }
                        // Check TLS certs and endpoint connectivity if tls: true
                        if cfg["tls"].as_bool().unwrap_or(false) {
                            let cert_path = matrix["tls"]["cert"].as_str();
                            let key_path_val = matrix["tls"]["key"].as_str();

                            match (cert_path, key_path_val) {
                                (None, _) => errors.push(format!("agent '{name}': tls.cert not set in YAML")),
                                (_, None) => errors.push(format!("agent '{name}': tls.key not set in YAML")),
                                (Some(cert), Some(key)) => {
                                    if !std::path::Path::new(cert).exists() {
                                        errors.push(format!("agent '{name}': TLS cert not found at {cert} (tls.cert in YAML)"));
                                    } else if !std::path::Path::new(key).exists() {
                                        errors.push(format!("agent '{name}': TLS key not found at {key} (tls.key in YAML)"));
                                    } else {
                                        ok.push(format!("agent '{name}': TLS certs present ({cert}, {key})"));
                                        if let Some(endpoint) = cfg["endpoint"].as_str() {
                                            let reachable = check_api_endpoint(endpoint, None, Some(cert), Some(key));
                                            if reachable {
                                                ok.push(format!("agent '{name}': TLS endpoint reachable"));
                                            } else {
                                                errors.push(format!("agent '{name}': TLS endpoint unreachable ({endpoint})"));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    errors.push("matrix: no agents defined".to_string());
                }

                // Validate judges
                if let Some(judges) = matrix["judges"].as_mapping() {
                    ok.push(format!("{} judges configured", judges.len()));
                    for (key, cfg) in judges {
                        let name = key.as_str().unwrap_or("?");
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
                                    let reachable = check_api_endpoint(endpoint, Some(&api_key), None, None);
                                    if reachable {
                                        ok.push(format!("judge '{name}': endpoint reachable"));
                                    } else {
                                        errors.push(format!("judge '{name}': endpoint unreachable ({endpoint})"));
                                    }
                                }
                            }
                        }
                    }
                } else {
                    errors.push("matrix: no judges defined".to_string());
                }

                // Validate prompt files
                if let Some(prompts) = matrix["prompts"].as_mapping() {
                    for (key, val) in prompts {
                        let name = key.as_str().unwrap_or("?");
                        if let Some(path) = val.as_str() {
                            let full_path = format!("{resources}/{path}");
                            if std::path::Path::new(&full_path).exists() {
                                ok.push(format!("prompt '{name}': {path}"));
                            } else {
                                errors.push(format!("prompt '{name}': file not found at {full_path}"));
                            }
                        }
                        // null means compiled-in default — always ok
                    }
                }
            }
            Err(e) => errors.push(format!("evaluation-matrix.yaml parse error: {e}")),
        },
        Err(e) => errors.push(format!("evaluation-matrix.yaml not found: {e}")),
    }

    // 3. Container runtime
    let has_docker = std::process::Command::new("docker").arg("info").output()
        .is_ok_and(|o| o.status.success());
    let has_podman = std::process::Command::new("podman").arg("info").output()
        .is_ok_and(|o| o.status.success());
    if has_docker {
        ok.push("container runtime: docker".to_string());
    } else if has_podman {
        ok.push("container runtime: podman".to_string());
    } else {
        errors.push("no container runtime: install docker or podman".to_string());
    }

    // 4. Filters (informational)
    for filter in &["FILTER_MODELS", "FILTER_PROMPTS", "FILTER_SCALES", "FILTER_NOISE", "FILTER_JUDGES"] {
        if let Ok(val) = std::env::var(filter) {
            warnings.push(format!("{filter}={val} (subset mode)"));
        }
    }

    PrecheckResult {
        pass: errors.is_empty(),
        ok,
        warnings,
        errors,
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn judge_prompt_evaluation_across_all_use_cases()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let resources = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/rehydration-testkit/resources");

    // ── Precheck: validate everything before booting containers ──
    let precheck = precheck(resources);
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

    let matrix: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(format!("{resources}/evaluation-matrix.yaml"))?,
    )?;

    let filter_models = env_filter("FILTER_MODELS");
    let filter_prompts = env_filter("FILTER_PROMPTS");
    let filter_scales = env_filter("FILTER_SCALES");
    let filter_noise = env_filter("FILTER_NOISE");

    // ── Phase 1: Boot + capture ──

    type ScaleEntry = (&'static str, fn(Domain) -> GraphSeedConfig);
    let scales: Vec<ScaleEntry> = vec![
        ("micro", |d| GraphSeedConfig::micro(d)),
        ("meso", |d| GraphSeedConfig::meso(d)),
        ("stress", |d| GraphSeedConfig::stress(d)),
    ];
    let domains = [("ops", Domain::Operations), ("debug", Domain::SoftwareDebugging)];
    let mixes = [("explanatory", RelationMix::Explanatory), ("structural", RelationMix::Structural), ("mixed", RelationMix::Mixed)];
    let noises = [("clean", NoiseMode::Structural), ("competing", NoiseMode::CompetingCausal)];

    let mut captured: Vec<CapturedVariant> = Vec::new();
    let mut fixture: Option<TestFixture> = None;

    for &(scale_name, scale_fn) in &scales {
        if !filter_scales.is_empty() && !filter_scales.contains(&scale_name.to_string()) {
            continue;
        }
        for &(domain_name, domain) in &domains {
            for &(mix_name, mix) in &mixes {
                for &(noise_name, noise_mode) in &noises {
                    if !filter_noise.is_empty() && !filter_noise.contains(&noise_name.to_string()) {
                        continue;
                    }
                let mut config = scale_fn(domain);
                config.relation_mix = mix;
                config.noise_mode = noise_mode;
                let variant_id = format!("{scale_name}-{domain_name}-{mix_name}-{noise_name}");
                config.id_prefix = variant_id.clone();

                let seed = generate_seed(config.clone());
                let events = seed_to_projection_events(&seed, SUBJECT_PREFIX, &variant_id)?;

                let root_id = seed.root.node_id.clone();
                let focus_id = seed.nodes.first()
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
                            client.publish(subject.clone(), payload.clone().into()).await?;
                        }
                        client.flush().await?;

                        wait_for_context(&f, &root_id, &focus_id).await?;
                        fixture = Some(f);
                    }
                    Some(f) => {
                        run.log(&format!("[INFRA] Reseeding {variant_id}..."));
                        let client = connect_nats_with_retry(f.nats_url()).await?;
                        for (subject, payload) in &events {
                            client.publish(subject.clone(), payload.clone().into()).await?;
                        }
                        client.flush().await?;

                        wait_for_context(f, &root_id, &focus_id).await?;
                    }
                }

                sleep(Duration::from_millis(500)).await;

                let mut qc = fixture.as_ref().expect("fixture should be Some").query_client();
                let response = qc.get_context(GetContextRequest {
                    root_node_id: root_id.clone(),
                    role: BENCHMARK_ROLE.to_string(),
                    phase: Phase::Build as i32,
                    work_item_id: focus_id.clone(),
                    token_budget: 4096,
                    requested_scopes: vec![],
                    render_format: BundleRenderFormat::Structured as i32,
                    include_debug_sections: false,
                    depth: config.chain_length as u32,
                    max_tier: 0,
                    rehydration_mode: 0,
                }).await?.into_inner();

                let rendered = response.rendered.ok_or("missing rendered")?;
                let eval_content = tier_content_for_eval(&rendered);
                let tokens = rendered.token_count;

                // Build ground truth that rewards multi-layer graph reasoning.
                //
                // The graph has layers:
                //   L0 (summary): root node kind + summary — surface info
                //   L1 (causal spine): chain-0 → chain-1 → ... → chain-N with rationales
                //   L2 (evidence): details, structural relations
                //
                // The chain is: root → chain-0 → chain-1 → ... → chain-N
                //
                // Ground truth rewards DEPTH of reasoning:
                //   - Failure point = leaf node (deepest chain node). A model that
                //     says "root" only read L0. A model that says "chain-N" traced L1.
                //   - Restart node = predecessor of the leaf. Shows the model
                //     understood the causal direction.
                //   - Reason = ALL chain rationales. Shows the model read L1 metadata,
                //     not just structural connections.
                let chain_nodes: Vec<&rehydration_testkit::GeneratedNode> = seed
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

                // Failure point: the leaf of the causal chain — proves L1 tracing.
                // The judge prompt already accepts causally connected nodes (line 62-64
                // of llm_prompts.yaml), so intermediate nodes are also valid.
                let failure_desc = if let Some(leaf) = chain_nodes.last() {
                    format!(
                        "{} ({}) \u{2014} {}. Causal chain: {} \u{2192} {}",
                        leaf.title,
                        leaf.node_id,
                        leaf.summary,
                        seed.root.title,
                        chain_nodes.iter().map(|n| n.title.as_str()).collect::<Vec<_>>().join(" \u{2192} ")
                    )
                } else {
                    format!("{} \u{2014} {} (single node, no chain)", seed.root.node_kind, seed.root.summary)
                };

                // Restart node: predecessor of the leaf — proves causal direction.
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

                // Reason: ALL chain rationales — proves L1 metadata was read.
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

                run.log(&format!("[CAPTURE] {variant_id}: {tokens} tokens, reason={}", reason.as_deref().unwrap_or("none")));

                captured.push(CapturedVariant {
                    run_id: variant_id,
                    rendered_content: eval_content,
                    rendered_tokens: tokens,
                    question,
                    ground_truth: EvaluationGroundTruth {
                        expected_failure_point: Some(failure_desc),
                        expected_restart_node: Some(restart_desc),
                        expected_reason: reason,
                        domain_context: Some(domain_name.to_string()),
                    },
                });
                }
            }
        }
    }

    let boot_ms = boot_start.elapsed().as_secs_f64() * 1000.0;
    run.log(&format!("[INFRA] {} variants captured in {boot_ms:.0}ms\n", captured.len()));

    // ── Phase 2: Evaluate model x prompt x captured variant ──

    let agents = matrix["agents"].as_mapping().expect("agents mapping");
    let judges = matrix["judges"].as_mapping().expect("judges mapping");
    let prompts = matrix["prompts"].as_mapping().expect("prompts mapping");

    let filter_judges = env_filter("FILTER_JUDGES");

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
            if agent_name == judge_name { continue; }

            let llm_config = build_llm_config(&matrix, agent_cfg, judge_cfg);

            for (prompt_key, prompt_val) in prompts {
                let prompt_name = prompt_key.as_str().expect("prompt key");
                if !filter_prompts.is_empty() && !filter_prompts.contains(&prompt_name.to_string()) {
                    continue;
                }

                let prompt_file = prompt_val.as_str().map(|p| format!("{resources}/{p}"));
                let prompts_cfg = PromptConfig::load(prompt_file.as_deref())?;

                run.log(&format!("\n\u{2588}\u{2588}\u{2588}\u{2588} {agent_name}\u{2192}{judge_name}/{prompt_name} \u{2588}\u{2588}\u{2588}\u{2588}"));

                for ctx in &captured {
                    eval_count += 1;
                    let eval = evaluate_with_config(
                        &prompts_cfg, &llm_config,
                        &ctx.rendered_content, &ctx.question, &ctx.ground_truth,
                    ).await;

                    let cell_id = format!("{agent_name}\u{2192}{judge_name}");
                    let result = match eval {
                        Ok(e) => {
                            let t = if e.llm_task_success { "OK" } else { "FAIL" };
                            let r = if e.llm_restart_accuracy { "OK" } else { "FAIL" };
                            let p = if e.llm_reason_preserved { "OK" } else { "FAIL" };
                            let resp_short: String = e.llm_response.replace('\n', " ").chars().take(120).collect();
                            run.log(&format!("  [{cell_id}/{prompt_name}] {}: T={t} R={r} P={p}  agent=\"{resp_short}...\"  judge={}",
                                ctx.run_id, e.llm_judge_raw.as_deref().unwrap_or("?")));
                            EvalResult {
                                model: cell_id,
                                prompt: prompt_name.to_string(),
                                variant: ctx.run_id.clone(),
                                task: Some(e.llm_task_success),
                                restart: Some(e.llm_restart_accuracy),
                                reason: Some(e.llm_reason_preserved),
                                latency_ms: e.llm_latency_ms,
                                agent_response: e.llm_response,
                                judge_raw: e.llm_judge_raw,
                            }
                        }
                        Err(err) => {
                            run.log(&format!("  [{cell_id}/{prompt_name}] {}: ERROR {err}", ctx.run_id));
                            EvalResult {
                                model: cell_id,
                                prompt: prompt_name.to_string(),
                                variant: ctx.run_id.clone(),
                                task: None, restart: None, reason: None,
                                latency_ms: 0.0,
                                agent_response: String::new(),
                                judge_raw: None,
                            }
                        }
                    };

                    let result_name = format!("{eval_count:04}_{agent_name}_{judge_name}_{prompt_name}_{}", ctx.run_id);
                    run.write_result(&result_name, &result)?;
                    results.push(result);
                }
            }
        }
    }

    let total_ms = boot_start.elapsed().as_secs_f64() * 1000.0;

    // ── Phase 3: Report ──

    run.log(&format!("\n\n{}", "=".repeat(120)));
    run.log(&format!("EVALUATION MATRIX \u{2014} {} variants captured, {} evals, boot {boot_ms:.0}ms, total {total_ms:.0}ms",
        captured.len(), results.len()));
    run.log(&format!("{}\n", "=".repeat(120)));

    for r in &results {
        let ctx = captured.iter().find(|c| c.run_id == r.variant);
        let question = ctx.map(|c| c.question.as_str()).unwrap_or("?");
        let expected = ctx.and_then(|c| c.ground_truth.expected_reason.as_deref()).unwrap_or("none");
        let tokens = ctx.map(|c| c.rendered_tokens).unwrap_or(0);

        let t = match r.task { Some(true) => "OK", Some(false) => "FAIL", None => "ERR" };
        let re = match r.restart { Some(true) => "OK", Some(false) => "FAIL", None => "ERR" };
        let p = match r.reason { Some(true) => "OK", Some(false) => "FAIL", None => "ERR" };

        run.log(&format!("\u{250c}\u{2500}\u{2500} {:<14} \u{00d7} {:<14} / {} ({tokens} tok) \u{2500}\u{2500}", r.model, r.prompt, r.variant));
        run.log(&format!("\u{2502} 1. Question:        {question}"));
        run.log(&format!("\u{2502} 2. Expected reason: {expected}"));
        let resp = r.agent_response.replace('\n', " ");
        run.log(&format!("\u{2502} 3. Agent response:  {resp}"));
        run.log(&format!("\u{2502} 4. Judge response:  {}", r.judge_raw.as_deref().unwrap_or("(none)")));
        run.log(&format!("\u{2502} 5. Result:          Task={t}  Restart={re}  Reason={p}  ({:.0}ms)", r.latency_ms));
        run.log("\u{2514}\u{2500}\u{2500}\n");
    }

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
    run.log(&format!("\u{2551}  {:<16} {:<16} {:>8} {:>8} {:>8} {:>8} \u{2551}",
        "Model", "Prompt", "Task", "Restart", "Reason", "LLM ms"));
    run.log(&format!("\u{2560}{}\u{2563}", "\u{2550}".repeat(78)));

    let mut seen: Vec<(String, String)> = Vec::new();
    for r in results {
        let key = (r.model.clone(), r.prompt.clone());
        if seen.contains(&key) { continue; }
        seen.push(key);

        let cell: Vec<&EvalResult> = results.iter()
            .filter(|x| x.model == r.model && x.prompt == r.prompt)
            .collect();
        let n = cell.iter().filter(|x| x.task.is_some()).count();
        let t = cell.iter().filter(|x| x.task == Some(true)).count();
        let re = cell.iter().filter(|x| x.restart == Some(true)).count();
        let p = cell.iter().filter(|x| x.reason == Some(true)).count();
        let ms: f64 = cell.iter().map(|x| x.latency_ms).sum();

        run.log(&format!("\u{2551}  {:<16} {:<16} {:>3}/{:<4} {:>3}/{:<4} {:>3}/{:<4} {:>6.0} \u{2551}",
            r.model, r.prompt, t, n, re, n, p, n, ms));
    }
    run.log(&format!("\u{255a}{}\u{255d}", "\u{2550}".repeat(78)));

    run.log("\nBy scale:");
    for scale in &["micro", "meso", "stress"] {
        let sr: Vec<&EvalResult> = results.iter().filter(|r| r.variant.starts_with(scale)).collect();
        if sr.is_empty() { continue; }
        let n = sr.iter().filter(|x| x.task.is_some()).count();
        let t = sr.iter().filter(|x| x.task == Some(true)).count();
        let p = sr.iter().filter(|x| x.reason == Some(true)).count();
        run.log(&format!("  {scale:<8}: Task {t}/{n}  Reason {p}/{n}"));
    }

    run.log("\nBy relation mix:");
    for mix in &["explanatory", "structural", "mixed"] {
        let mr: Vec<&EvalResult> = results.iter().filter(|r| r.variant.contains(mix)).collect();
        if mr.is_empty() { continue; }
        let n = mr.iter().filter(|x| x.task.is_some()).count();
        let t = mr.iter().filter(|x| x.task == Some(true)).count();
        let p = mr.iter().filter(|x| x.reason == Some(true)).count();
        run.log(&format!("  {mix:<14}: Task {t}/{n}  Reason {p}/{n}"));
    }
}

/// Checks that an LLM endpoint is reachable with a real HTTP request.
/// For OpenAI-compatible: GET /v1/models. For Anthropic: GET /v1/models (returns 404 but proves connectivity).
fn check_api_endpoint(endpoint: &str, api_key: Option<&str>, tls_cert: Option<&str>, tls_key: Option<&str>) -> bool {
    let probe_url = endpoint
        .replace("/v1/chat/completions", "/v1/models")
        .replace("/v1/messages", "/v1/models");

    let mut args = vec!["-s", "--max-time", "10", "-o", "/dev/null", "-w", "%{http_code}"];
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
        other => panic!("unknown provider '{other}' in YAML — expected: openai, openai-new, anthropic"),
    }
}

fn yaml_str(cfg: &serde_yaml::Value, field: &str, context: &str) -> String {
    cfg[field]
        .as_str()
        .unwrap_or_else(|| panic!("{context}: missing required field '{field}' in YAML"))
        .to_string()
}

fn build_llm_config(matrix: &serde_yaml::Value, agent_cfg: &serde_yaml::Value, judge_cfg: &serde_yaml::Value) -> LlmEvaluatorConfig {
    let tls = agent_cfg["tls"].as_bool().unwrap_or(false);
    let tls_section = &matrix["tls"];
    let tls_cert = if tls { Some(yaml_str(tls_section, "cert", "tls")) } else { None };
    let tls_key = if tls { Some(yaml_str(tls_section, "key", "tls")) } else { None };

    LlmEvaluatorConfig {
        endpoint: yaml_str(agent_cfg, "endpoint", "agent"),
        model: yaml_str(agent_cfg, "model", "agent"),
        provider: parse_provider(&yaml_str(agent_cfg, "provider", "agent")),
        api_key: agent_cfg["api_key_env"].as_str().and_then(|e| std::env::var(e).ok()),
        max_tokens: 200,
        temperature: 0.0,
        tls_cert_path: tls_cert,
        tls_key_path: tls_key,
        tls_insecure: tls,
        judge_endpoint: Some(yaml_str(judge_cfg, "endpoint", "judge")),
        judge_model: Some(yaml_str(judge_cfg, "model", "judge")),
        judge_provider: Some(parse_provider(&yaml_str(judge_cfg, "provider", "judge"))),
        judge_api_key: judge_cfg["api_key_env"].as_str().and_then(|e| std::env::var(e).ok()),
    }
}

async fn wait_for_context(
    fixture: &TestFixture,
    root_node_id: &str,
    focus_node_id: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut qc = fixture.query_client();
    for _ in 0..40 {
        if let Ok(resp) = qc.get_context(GetContextRequest {
            root_node_id: root_node_id.to_string(),
            role: BENCHMARK_ROLE.to_string(),
            phase: Phase::Build as i32,
            work_item_id: focus_node_id.to_string(),
            token_budget: 1200,
            requested_scopes: vec!["implementation".to_string()],
            render_format: BundleRenderFormat::Structured as i32,
            include_debug_sections: false,
            depth: 0,
            max_tier: 0,
            rehydration_mode: 0,
        }).await {
            let resp = resp.into_inner();
            if let Some(bundle) = resp.bundle
                && bundle.root_node_id == root_node_id
                && bundle.bundles.first().is_some_and(|b| !b.neighbor_nodes.is_empty())
            {
                return Ok(());
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err("context did not become ready".into())
}
