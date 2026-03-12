use std::path::Path;

use rehydration_proto::v1alpha1::{
    context_admin_service_client::ContextAdminServiceClient,
    context_query_service_client::ContextQueryServiceClient,
};
use tokio::time::sleep;

use crate::logging::debug_log_value;
use crate::runtime_contract::{AgentRuntime, RuntimeResult};
use crate::{
    FileSystemRuntime, LlmPlanner, LlmStarshipMissionAgent, LlmStarshipMissionExecution,
    LlmStarshipMissionRequest, StarshipDemoConfig, StarshipDemoPhaseSummary,
    StarshipDemoProviderSummary, StarshipDemoSummary, StarshipRuntimeMode, StarshipScenario,
    UnderpassRuntimeClient,
};

struct DemoExecutionContext<R> {
    query_client: ContextQueryServiceClient<tonic::transport::Channel>,
    admin_client: ContextAdminServiceClient<tonic::transport::Channel>,
    runtime: R,
    llm: LlmPlanner,
    request: LlmStarshipMissionRequest,
    provider: StarshipDemoProviderSummary,
}

pub async fn run_starship_demo(
    config: StarshipDemoConfig,
) -> Result<StarshipDemoSummary, Box<dyn std::error::Error + Send + Sync>> {
    prepare_workspace(&config.workspace_dir, config.reset_workspace)?;

    let scenario = StarshipScenario::for_run_id(config.run_id.clone());
    let llm = LlmPlanner::from_env()?;
    let kernel_grpc_endpoint = config.kernel_grpc_endpoint.clone();
    let query_client = ContextQueryServiceClient::connect(kernel_grpc_endpoint.clone()).await?;
    let admin_client = ContextAdminServiceClient::connect(kernel_grpc_endpoint).await?;
    let publisher = async_nats::connect(config.nats_url.clone()).await?;
    scenario
        .publish_initial_projection_events(&publisher)
        .await?;

    let request = LlmStarshipMissionRequest::reference_defaults(
        scenario.root_node_id(),
        scenario.root_node_kind(),
    );
    let provider = StarshipDemoProviderSummary {
        llm_provider: config.llm_provider.clone(),
        runtime_mode: runtime_mode_label(config.runtime_mode).to_string(),
    };

    match config.runtime_mode {
        StarshipRuntimeMode::FileSystem => {
            let runtime = FileSystemRuntime::new(&config.workspace_dir);
            run_with_runtime(
                &config,
                &scenario,
                &publisher,
                DemoExecutionContext {
                    query_client,
                    admin_client,
                    runtime,
                    llm,
                    request,
                    provider,
                },
            )
            .await
        }
        StarshipRuntimeMode::Http => {
            let runtime = UnderpassRuntimeClient::connect(
                config
                    .runtime_base_url
                    .clone()
                    .expect("runtime_base_url is required for http mode"),
            )
            .await?;
            run_with_runtime(
                &config,
                &scenario,
                &publisher,
                DemoExecutionContext {
                    query_client,
                    admin_client,
                    runtime,
                    llm,
                    request,
                    provider,
                },
            )
            .await
        }
    }
}

async fn run_with_runtime<R>(
    config: &StarshipDemoConfig,
    scenario: &StarshipScenario,
    publisher: &async_nats::Client,
    context: DemoExecutionContext<R>,
) -> Result<StarshipDemoSummary, Box<dyn std::error::Error + Send + Sync>>
where
    R: AgentRuntime + Clone,
{
    let DemoExecutionContext {
        query_client,
        admin_client,
        runtime,
        llm,
        request,
        provider,
    } = context;

    let mut phase_one_agent = LlmStarshipMissionAgent::new(
        query_client.clone(),
        admin_client.clone(),
        runtime.clone(),
        llm.clone(),
    );
    wait_for_current_step(
        &mut phase_one_agent,
        &request,
        scenario.step_one_node_id(),
        config.wait_attempts,
        config.wait_poll_interval,
    )
    .await?;
    let phase_one = phase_one_agent.execute_next_step(request.clone()).await?;

    scenario.publish_resume_projection_events(publisher).await?;

    let mut phase_two_agent =
        LlmStarshipMissionAgent::new(query_client, admin_client, runtime, llm);
    wait_for_current_step(
        &mut phase_two_agent,
        &request,
        scenario.step_two_node_id(),
        config.wait_attempts,
        config.wait_poll_interval,
    )
    .await?;
    let phase_two = phase_two_agent.execute_next_step(request).await?;

    let workspace_dir = config.workspace_dir.to_string_lossy().to_string();
    debug_log_value("starship demo workspace", &workspace_dir);
    Ok(StarshipDemoSummary {
        run_id: scenario.run_id().to_string(),
        root_node_id: scenario.root_node_id().to_string(),
        workspace_dir,
        provider,
        phase_one: phase_summary(&phase_one),
        phase_two: phase_summary(&phase_two),
        captains_log: phase_two.captains_log,
    })
}

async fn wait_for_current_step<R>(
    agent: &mut LlmStarshipMissionAgent<R>,
    request: &LlmStarshipMissionRequest,
    expected_step_node_id: &str,
    attempts: usize,
    poll_interval: std::time::Duration,
) -> RuntimeResult<()>
where
    R: AgentRuntime,
{
    for _ in 0..attempts {
        match agent.current_step(request).await {
            Ok(step) => {
                if step.node_id == expected_step_node_id {
                    return Ok(());
                }
            }
            Err(error) => {
                if !error.to_string().contains("Node not found") {
                    return Err(error);
                }
            }
        }
        sleep(poll_interval).await;
    }

    Err(
        format!("expected current step `{expected_step_node_id}` after starship rehydration")
            .into(),
    )
}

fn prepare_workspace(path: &Path, reset_workspace: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    if reset_workspace {
        clear_workspace_contents(path)?;
    }
    Ok(())
}

fn clear_workspace_contents(path: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            std::fs::remove_dir_all(entry_path)?;
        } else {
            std::fs::remove_file(entry_path)?;
        }
    }
    Ok(())
}

fn phase_summary(phase: &LlmStarshipMissionExecution) -> StarshipDemoPhaseSummary {
    StarshipDemoPhaseSummary {
        step_node_id: phase.selected_step_node_id.clone(),
        written_paths: phase.written_paths.clone(),
    }
}

fn runtime_mode_label(runtime_mode: StarshipRuntimeMode) -> &'static str {
    match runtime_mode {
        StarshipRuntimeMode::FileSystem => "filesystem",
        StarshipRuntimeMode::Http => "http",
    }
}
