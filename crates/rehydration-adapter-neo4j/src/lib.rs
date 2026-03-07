use rehydration_domain::{CaseId, RehydrationBundle, Role};
use rehydration_ports::{PortError, ProjectionReader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neo4jProjectionReader {
    endpoint: Neo4jEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Neo4jEndpoint {
    graph_uri: String,
}

impl Neo4jProjectionReader {
    pub fn new(graph_uri: impl Into<String>) -> Result<Self, PortError> {
        let endpoint = Neo4jEndpoint::parse(graph_uri.into())?;
        Ok(Self { endpoint })
    }
}

impl ProjectionReader for Neo4jProjectionReader {
    async fn load_bundle(
        &self,
        _case_id: &CaseId,
        _role: &Role,
    ) -> Result<Option<RehydrationBundle>, PortError> {
        // The projection model is not implemented yet. Returning `None` keeps the
        // adapter honest and prevents infrastructure from inventing domain data.
        let _endpoint = &self.endpoint;
        Ok(None)
    }
}

impl Neo4jEndpoint {
    fn parse(graph_uri: String) -> Result<Self, PortError> {
        if graph_uri.trim().is_empty() {
            return Err(PortError::InvalidState(
                "graph uri cannot be empty".to_string(),
            ));
        }

        let (scheme, authority) = split_uri(&graph_uri, "graph")?;
        if !matches!(
            scheme,
            "neo4j" | "neo4j+s" | "neo4j+ssc" | "bolt" | "bolt+s" | "bolt+ssc"
        ) {
            return Err(PortError::InvalidState(format!(
                "unsupported graph scheme `{scheme}`"
            )));
        }

        parse_authority(authority, "graph")?;

        Ok(Self { graph_uri })
    }
}

fn split_uri<'a>(raw_uri: &'a str, name: &str) -> Result<(&'a str, &'a str), PortError> {
    let (scheme, remainder) = raw_uri
        .split_once("://")
        .ok_or_else(|| PortError::InvalidState(format!("{name} uri must include a scheme")))?;
    if scheme.is_empty() {
        return Err(PortError::InvalidState(format!(
            "{name} uri must include a scheme"
        )));
    }

    let authority = remainder
        .split(['/', '?'])
        .next()
        .unwrap_or_default()
        .trim();
    if authority.is_empty() {
        return Err(PortError::InvalidState(format!(
            "{name} uri must include a host"
        )));
    }

    Ok((scheme, authority))
}

fn parse_authority(authority: &str, name: &str) -> Result<(), PortError> {
    if authority.contains('@') {
        return Err(PortError::InvalidState(format!(
            "{name} uri auth segments are not supported yet"
        )));
    }

    if authority.starts_with('[') {
        let (_, remainder) = authority.split_once(']').ok_or_else(|| {
            PortError::InvalidState(format!("{name} uri contains an invalid IPv6 host"))
        })?;
        if !remainder.is_empty() && !remainder.starts_with(':') {
            return Err(PortError::InvalidState(format!(
                "{name} uri contains an invalid port separator"
            )));
        }

        return Ok(());
    }

    if let Some((host, port)) = authority.rsplit_once(':') {
        if host.is_empty() {
            return Err(PortError::InvalidState(format!(
                "{name} uri must include a host"
            )));
        }

        if !port.is_empty() {
            port.parse::<u16>().map_err(|error| {
                PortError::InvalidState(format!("{name} uri contains an invalid port: {error}"))
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rehydration_domain::{CaseId, Role};
    use rehydration_ports::ProjectionReader;

    use super::Neo4jProjectionReader;

    #[tokio::test]
    async fn adapter_does_not_invent_projection_data() {
        let reader =
            Neo4jProjectionReader::new("neo4j://localhost:7687").expect("uri should be accepted");
        let bundle = reader
            .load_bundle(
                &CaseId::new("case-123").expect("case id is valid"),
                &Role::new("developer").expect("role is valid"),
            )
            .await
            .expect("bundle load should succeed");

        assert!(bundle.is_none());
    }

    #[test]
    fn adapter_rejects_invalid_scheme() {
        let error = Neo4jProjectionReader::new("https://localhost:7687")
            .expect_err("unsupported schemes must fail");

        assert_eq!(
            error,
            rehydration_ports::PortError::InvalidState(
                "unsupported graph scheme `https`".to_string()
            )
        );
    }
}
