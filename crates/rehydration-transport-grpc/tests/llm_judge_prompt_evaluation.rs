#![cfg(feature = "container-tests")]
#![allow(deprecated)]

mod agentic_support;

use std::error::Error;
use std::time::{Duration, Instant};

use agentic_support::agentic_fixture::AgenticFixture;
use rehydration_proto::v1beta1::{
    BundleRenderFormat, GetContextRequest, Phase, RenderedContext, ResolutionTier,
    context_query_service_client::ContextQueryServiceClient,
};
use rehydration_testkit::{
    Domain, EvaluationGroundTruth, GraphSeedConfig, LlmEvaluatorConfig, LlmProvider,
    NoiseMode, PromptConfig, RelationMix, evaluate_with_config, generate_seed,
    seed_publisher::seed_to_projection_events,
};
use tokio::time::sleep;
use tonic::transport::Channel;

const SUBJECT_PREFIX: &str = "rehydration";
const BENCHMARK_ROLE: &str = "evaluator";

/// A captured graph context — fixed text that all model×prompt cells evaluate.
struct CapturedVariant {
    run_id: String,
    scale: &'static str,
    domain: &'static str,
    mix: &'static str,
    rendered_content: String,
    rendered_tokens: u32,
    question: String,
    ground_truth: EvaluationGroundTruth,
}

/// Result of one model×prompt×variant evaluation.
struct CellResult {
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

/// Single-process matrix evaluation:
///
/// Phase 1: Boot containers once, generate graphs at each scale, capture
///          rendered context via real gRPC. All variants captured upfront.
///
/// Phase 2: For each model × prompt cell, evaluate ALL captured variants
///          using the exact same rendered text. No re-seeding, no re-booting.
///
/// Filter with env vars:
///   FILTER_MODELS=qwen3-8b,gpt-5.4
///   FILTER_PROMPTS=default,citation-agent
///   FILTER_SCALES=micro,meso
///
/// API keys: ANTHROPIC_KEY, OPENAI_KEY
#[tokio::test]
async fn judge_prompt_evaluation_across_all_use_cases()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let boot_start = Instant::now();

    // ── Load matrix config ──
    let resources = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/rehydration-testkit/resources");
    let matrix: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(format!("{resources}/evaluation-matrix.yaml"))?,
    )?;

    let filter_models = env_filter("FILTER_MODELS");
    let filter_prompts = env_filter("FILTER_PROMPTS");
    let filter_scales = env_filter("FILTER_SCALES");

    // ── Phase 1: Boot + capture ──

    let scales: Vec<(&str, fn(Domain) -> GraphSeedConfig)> = vec![
        ("micro", |d| GraphSeedConfig::micro(d)),
        ("meso", |d| GraphSeedConfig::meso(d)),
        ("stress", |d| GraphSeedConfig::stress(d)),
    ];
    let domains = [("ops", Domain::Operations), ("debug", Domain::SoftwareDebugging)];
    let mixes = [("explanatory", RelationMix::Explanatory), ("structural", RelationMix::Structural), ("mixed", RelationMix::Mixed)];
    let noises = [("clean", NoiseMode::Structural), ("competing", NoiseMode::CompetingCausal)];

    let filter_noise = env_filter("FILTER_NOISE");

    let mut captured: Vec<CapturedVariant> = Vec::new();
    let mut fixture: Option<AgenticFixture> = None;

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
                let run_id = format!("{scale_name}-{domain_name}-{mix_name}-{noise_name}");
                config.id_prefix = run_id.clone();

                let seed = generate_seed(config.clone());
                let events = seed_to_projection_events(&seed, SUBJECT_PREFIX, &run_id)?;

                let root_id = seed.root.node_id.clone();
                let focus_id = seed.nodes.first()
                    .map(|n| n.node_id.clone())
                    .unwrap_or_else(|| root_id.clone());

                // Boot or reseed
                match &fixture {
                    None => {
                        eprintln!("[INFRA] Booting containers for {run_id}...");
                        fixture = Some(AgenticFixture::start_with_seed(
                            &root_id, &focus_id,
                            |publisher| {
                                let events = events.clone();
                                async move {
                                    for (subject, payload) in events {
                                        publisher.publish(subject, payload.into()).await?;
                                    }
                                    publisher.flush().await?;
                                    Ok(())
                                }
                            },
                        ).await?);
                    }
                    Some(f) => {
                        eprintln!("[INFRA] Reseeding {run_id}...");
                        f.reseed(&root_id, &focus_id, |publisher| {
                            let events = events.clone();
                            async move {
                                for (subject, payload) in events {
                                    publisher.publish(subject, payload.into()).await?;
                                }
                                publisher.flush().await?;
                                Ok(())
                            }
                        }).await?;
                    }
                }

                sleep(Duration::from_millis(500)).await;

                // Query context
                let mut qc = fixture.as_ref().unwrap().query_client();
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

                // Build ground truth from seed
                let first_chain = seed.nodes.first();
                let last_chain = seed.nodes.iter().filter(|n| n.node_kind != "distractor").last();

                let question = format!(
                    "Given this rehydrated context from a {} graph:\n\
                     1. What is the root cause or failure point?\n\
                     2. Which node should the system restart from to continue work?\n\
                     3. What is the main rationale in the causal chain?",
                    domain_name
                );

                let failure_desc = format!(
                    "{} — {}",
                    seed.root.node_kind, seed.root.summary
                );
                let restart_desc = format!(
                    "Any node on the main chain: {} or {}",
                    first_chain.map(|n| n.title.as_str()).unwrap_or("first"),
                    last_chain.map(|n| n.title.as_str()).unwrap_or("last"),
                );
                let reason = seed.relations.first().and_then(|r| r.rationale.clone());

                eprintln!("[CAPTURE] {run_id}: {tokens} tokens, reason={}", reason.as_deref().unwrap_or("none"));

                captured.push(CapturedVariant {
                    run_id,
                    scale: scale_name,
                    domain: domain_name,
                    mix: mix_name,
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
    eprintln!("[INFRA] {} variants captured in {boot_ms:.0}ms\n", captured.len());

    // ── Phase 2: Evaluate model × prompt × captured variant ──

    let agents = matrix["agents"].as_mapping().unwrap();
    let judges = matrix["judges"].as_mapping().unwrap();
    let prompts = matrix["prompts"].as_mapping().unwrap();

    let filter_judges = env_filter("FILTER_JUDGES");

    let mut results: Vec<CellResult> = Vec::new();

    for (agent_key, agent_cfg) in agents {
        let agent_name = agent_key.as_str().unwrap();
        if !filter_models.is_empty() && !filter_models.contains(&agent_name.to_string()) {
            continue;
        }

        for (judge_key, judge_cfg) in judges {
            let judge_name = judge_key.as_str().unwrap();
            if !filter_judges.is_empty() && !filter_judges.contains(&judge_name.to_string()) {
                continue;
            }
            // Skip self-judging
            if agent_name == judge_name { continue; }

            let llm_config = build_llm_config(agent_cfg, judge_cfg);

            for (prompt_key, prompt_val) in prompts {
                let prompt_name = prompt_key.as_str().unwrap();
                if !filter_prompts.is_empty() && !filter_prompts.contains(&prompt_name.to_string()) {
                    continue;
                }

                let prompt_file = prompt_val.as_str().map(|p| format!("{resources}/{p}"));
                let prompts_cfg = PromptConfig::load(prompt_file.as_deref())?;

                let cell_label = format!("{agent_name}→{judge_name}/{prompt_name}");
                eprintln!("████ {cell_label} ████");

                for ctx in &captured {
                    let eval = evaluate_with_config(
                        &prompts_cfg, &llm_config,
                        &ctx.rendered_content, &ctx.question, &ctx.ground_truth,
                    ).await;

                    let cell_id = format!("{agent_name}→{judge_name}");
                    match eval {
                        Ok(e) => {
                            let t = if e.llm_task_success { "OK" } else { "FAIL" };
                            let r = if e.llm_restart_accuracy { "OK" } else { "FAIL" };
                            let p = if e.llm_reason_preserved { "OK" } else { "FAIL" };
                            let resp_short: String = e.llm_response.replace('\n', " ").chars().take(120).collect();
                            eprintln!("  [{cell_id}/{prompt_name}] {}: T={t} R={r} P={p}  agent=\"{resp_short}...\"  judge={}",
                                ctx.run_id, e.llm_judge_raw.as_deref().unwrap_or("?"));
                            results.push(CellResult {
                                model: cell_id,
                                prompt: prompt_name.to_string(),
                                variant: ctx.run_id.clone(),
                                task: Some(e.llm_task_success),
                                restart: Some(e.llm_restart_accuracy),
                                reason: Some(e.llm_reason_preserved),
                                latency_ms: e.llm_latency_ms,
                                agent_response: e.llm_response,
                                judge_raw: e.llm_judge_raw,
                            });
                        }
                        Err(err) => {
                            eprintln!("  [{cell_id}/{prompt_name}] {}: ERROR {err}", ctx.run_id);
                            results.push(CellResult {
                                model: cell_id,
                                prompt: prompt_name.to_string(),
                                variant: ctx.run_id.clone(),
                                task: None, restart: None, reason: None,
                                latency_ms: 0.0,
                                agent_response: String::new(),
                                judge_raw: None,
                            });
                        }
                    }
                }
                eprintln!();
            }
        }
    }

    // ── Phase 3: Report — every row, every field ──

    let total_ms = boot_start.elapsed().as_secs_f64() * 1000.0;

    eprintln!("\n\n{}", "=".repeat(120));
    eprintln!("EVALUATION MATRIX — {} variants captured, {} evals, boot {boot_ms:.0}ms, total {total_ms:.0}ms",
        captured.len(), results.len());
    eprintln!("{}\n", "=".repeat(120));

    // Find the matching captured context for each result to show the question
    for r in &results {
        let ctx = captured.iter().find(|c| c.run_id == r.variant);
        let question = ctx.map(|c| c.question.as_str()).unwrap_or("?");
        let expected = ctx.and_then(|c| c.ground_truth.expected_reason.as_deref()).unwrap_or("none");
        let tokens = ctx.map(|c| c.rendered_tokens).unwrap_or(0);

        let t = match r.task { Some(true) => "OK", Some(false) => "FAIL", None => "ERR" };
        let re = match r.restart { Some(true) => "OK", Some(false) => "FAIL", None => "ERR" };
        let p = match r.reason { Some(true) => "OK", Some(false) => "FAIL", None => "ERR" };

        eprintln!("┌── {:<14} × {:<14} / {} ({tokens} tok) ──", r.model, r.prompt, r.variant);
        eprintln!("│ 1. Question:        {question}");
        eprintln!("│ 2. Expected reason: {expected}");
        let resp = r.agent_response.replace('\n', " ");
        eprintln!("│ 3. Agent response:  {resp}");
        eprintln!("│ 4. Judge response:  {}", r.judge_raw.as_deref().unwrap_or("(none)"));
        eprintln!("│ 5. Result:          Task={t}  Restart={re}  Reason={p}  ({:.0}ms)", r.latency_ms);
        eprintln!("└──\n");
    }

    // Summary table
    eprintln!("╔══════════════════════════════════════════════════════════════════════════════╗");
    eprintln!("║  SUMMARY                                                                    ║");
    eprintln!("╠══════════════════════════════════════════════════════════════════════════════╣");
    eprintln!("║  {:<16} {:<16} {:>8} {:>8} {:>8} {:>8} ║",
        "Model", "Prompt", "Task", "Restart", "Reason", "LLM ms");
    eprintln!("╠══════════════════════════════════════════════════════════════════════════════╣");

    let mut seen: Vec<(String, String)> = Vec::new();
    for r in &results {
        let key = (r.model.clone(), r.prompt.clone());
        if seen.contains(&key) { continue; }
        seen.push(key);

        let cell: Vec<&CellResult> = results.iter()
            .filter(|x| x.model == r.model && x.prompt == r.prompt)
            .collect();
        let n = cell.iter().filter(|x| x.task.is_some()).count();
        let t = cell.iter().filter(|x| x.task == Some(true)).count();
        let re = cell.iter().filter(|x| x.restart == Some(true)).count();
        let p = cell.iter().filter(|x| x.reason == Some(true)).count();
        let ms: f64 = cell.iter().map(|x| x.latency_ms).sum();

        eprintln!("║  {:<16} {:<16} {:>3}/{:<4} {:>3}/{:<4} {:>3}/{:<4} {:>6.0} ║",
            r.model, r.prompt, t, n, re, n, p, n, ms);
    }
    eprintln!("╚══════════════════════════════════════════════════════════════════════════════╝");

    // By scale
    eprintln!("\nBy scale:");
    for scale in &["micro", "meso", "stress"] {
        let sr: Vec<&CellResult> = results.iter().filter(|r| r.variant.starts_with(scale)).collect();
        if sr.is_empty() { continue; }
        let n = sr.iter().filter(|x| x.task.is_some()).count();
        let t = sr.iter().filter(|x| x.task == Some(true)).count();
        let p = sr.iter().filter(|x| x.reason == Some(true)).count();
        eprintln!("  {scale:<8}: Task {t}/{n}  Reason {p}/{n}");
    }

    // By mix
    eprintln!("\nBy relation mix:");
    for mix in &["explanatory", "structural", "mixed"] {
        let mr: Vec<&CellResult> = results.iter().filter(|r| r.variant.contains(mix)).collect();
        if mr.is_empty() { continue; }
        let n = mr.iter().filter(|x| x.task.is_some()).count();
        let t = mr.iter().filter(|x| x.task == Some(true)).count();
        let p = mr.iter().filter(|x| x.reason == Some(true)).count();
        eprintln!("  {mix:<14}: Task {t}/{n}  Reason {p}/{n}");
    }

    if let Some(f) = fixture {
        f.shutdown().await?;
    }
    Ok(())
}

fn env_filter(key: &str) -> Vec<String> {
    std::env::var(key)
        .map(|s| s.split(',').map(str::trim).map(String::from).collect())
        .unwrap_or_default()
}

fn parse_provider(s: &str) -> LlmProvider {
    match s {
        "anthropic" => LlmProvider::Anthropic,
        "openai-new" => LlmProvider::OpenAINew,
        _ => LlmProvider::OpenAI,
    }
}

fn build_llm_config(agent_cfg: &serde_yaml::Value, judge_cfg: &serde_yaml::Value) -> LlmEvaluatorConfig {
    let tls = agent_cfg["tls"].as_bool().unwrap_or(false);

    LlmEvaluatorConfig {
        endpoint: agent_cfg["endpoint"].as_str().unwrap_or("").to_string(),
        model: agent_cfg["model"].as_str().unwrap_or("").to_string(),
        provider: parse_provider(agent_cfg["provider"].as_str().unwrap_or("openai")),
        api_key: agent_cfg["api_key_env"].as_str().and_then(|e| std::env::var(e).ok()),
        max_tokens: 200,
        temperature: 0.0,
        tls_cert_path: if tls { Some("/tmp/vllm-client.crt".into()) } else { None },
        tls_key_path: if tls { Some("/tmp/vllm-client.key".into()) } else { None },
        tls_insecure: tls,
        judge_endpoint: Some(judge_cfg["endpoint"].as_str().unwrap_or("").to_string()),
        judge_model: Some(judge_cfg["model"].as_str().unwrap_or("").to_string()),
        judge_provider: Some(parse_provider(judge_cfg["provider"].as_str().unwrap_or("anthropic"))),
        judge_api_key: judge_cfg["api_key_env"].as_str().and_then(|e| std::env::var(e).ok()),
    }
}
