use std::error::Error;
use std::time::Duration;

use neo4rs::{Graph, query};
use testcontainers::{
    GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tokio::time::sleep;

pub(crate) const NEO4J_INTERNAL_PORT: u16 = 7687;
pub(crate) const NEO4J_IMAGE: &str = "docker.io/neo4j";
pub(crate) const NEO4J_TAG: &str = "5.26.0-community";
pub(crate) const NEO4J_PASSWORD: &str = "underpass-test-password";
pub(crate) const VALKEY_INTERNAL_PORT: u16 = 6379;

pub(crate) async fn start_neo4j_container()
-> Result<testcontainers::ContainerAsync<GenericImage>, Box<dyn Error + Send + Sync>> {
    Ok(GenericImage::new(NEO4J_IMAGE, NEO4J_TAG)
        .with_exposed_port(NEO4J_INTERNAL_PORT.tcp())
        .with_env_var("NEO4J_AUTH", format!("neo4j/{NEO4J_PASSWORD}"))
        .start()
        .await?)
}

pub(crate) async fn start_valkey_container()
-> Result<testcontainers::ContainerAsync<GenericImage>, Box<dyn Error + Send + Sync>> {
    Ok(GenericImage::new("docker.io/valkey/valkey", "8.1.5-alpine")
        .with_exposed_port(VALKEY_INTERNAL_PORT.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .start()
        .await?)
}

pub(crate) async fn connect_with_retry(
    uri: String,
    user: &str,
    password: &str,
) -> Result<Graph, Box<dyn Error + Send + Sync>> {
    let mut last_error: Option<Box<dyn Error + Send + Sync>> = None;

    for _ in 0..30 {
        match Graph::new(&uri, user, password).await {
            Ok(graph) => return Ok(graph),
            Err(error) => {
                last_error = Some(Box::new(error));
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Err(last_error.expect("at least one connection attempt should fail"))
}

pub(crate) async fn clear_neo4j(uri: String) -> Result<(), Box<dyn Error + Send + Sync>> {
    let graph = connect_with_retry(uri, "neo4j", NEO4J_PASSWORD).await?;
    graph.run(query("MATCH (n) DETACH DELETE n")).await?;
    Ok(())
}
