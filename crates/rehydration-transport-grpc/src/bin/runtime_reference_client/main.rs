mod config;

use serde_json::json;

use crate::config::AppConfig;
use rehydration_proto::v1beta1::{
    context_admin_service_client::ContextAdminServiceClient,
    context_query_service_client::ContextQueryServiceClient,
};
use rehydration_transport_grpc::agentic_reference::{BasicContextAgent, UnderpassRuntimeClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env()?;
    let request = config.request.clone();

    let query_client =
        ContextQueryServiceClient::connect(config.kernel_grpc_endpoint.clone()).await?;
    let admin_client = ContextAdminServiceClient::connect(config.kernel_grpc_endpoint).await?;
    let runtime = UnderpassRuntimeClient::connect(config.runtime_base_url).await?;
    let mut agent = BasicContextAgent::new(query_client, admin_client, runtime);
    let execution = agent.execute(request.clone()).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "root_node_id": request.root_node_id,
            "root_node_kind": request.root_node_kind,
            "role": request.role,
            "phase": format!("{:?}", request.phase),
            "focus_node_kind": request.focus_node_kind,
            "requested_scopes": request.requested_scopes,
            "token_budget": request.token_budget,
            "summary_path": execution.written_path,
            "selected_node_id": execution.selected_node_id,
            "listed_files": execution.listed_files,
            "written_content": execution.written_content,
        }))?
    );

    Ok(())
}
