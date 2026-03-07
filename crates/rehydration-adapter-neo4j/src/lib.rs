use rehydration_domain::{CaseId, Role, RoleContextPack};
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
    async fn load_pack(
        &self,
        _case_id: &CaseId,
        _role: &Role,
    ) -> Result<Option<RoleContextPack>, PortError> {
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

    use super::{Neo4jProjectionReader, parse_authority, split_uri};

    #[tokio::test]
    async fn adapter_does_not_invent_projection_data() {
        let reader =
            Neo4jProjectionReader::new("neo4j://localhost:7687").expect("uri should be accepted");
        let bundle = reader
            .load_pack(
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

    #[test]
    fn adapter_rejects_missing_scheme_and_host() {
        let missing_scheme =
            Neo4jProjectionReader::new("localhost:7687").expect_err("scheme is required");
        let missing_host = Neo4jProjectionReader::new("neo4j://").expect_err("host is required");

        assert_eq!(
            missing_scheme,
            rehydration_ports::PortError::InvalidState(
                "graph uri must include a scheme".to_string()
            )
        );
        assert_eq!(
            missing_host,
            rehydration_ports::PortError::InvalidState("graph uri must include a host".to_string())
        );
    }

    #[test]
    fn parser_rejects_unsupported_authorities() {
        let auth_segment =
            parse_authority("user@localhost:7687", "graph").expect_err("auth is not supported");
        let invalid_separator = parse_authority("[::1]7687", "graph")
            .expect_err("ipv6 port separator must be explicit");
        let invalid_port =
            parse_authority("localhost:not-a-port", "graph").expect_err("port must be numeric");

        assert_eq!(
            auth_segment,
            rehydration_ports::PortError::InvalidState(
                "graph uri auth segments are not supported yet".to_string()
            )
        );
        assert_eq!(
            invalid_separator,
            rehydration_ports::PortError::InvalidState(
                "graph uri contains an invalid port separator".to_string()
            )
        );
        assert!(
            invalid_port
                .to_string()
                .starts_with("graph uri contains an invalid port:")
        );
    }

    #[test]
    fn parser_accepts_ipv6_and_splits_query_away() {
        let (scheme, authority) =
            split_uri("neo4j://[::1]:7687/path?tls=true", "graph").expect("uri should parse");
        parse_authority(authority, "graph").expect("ipv6 authority should be valid");

        assert_eq!(scheme, "neo4j");
        assert_eq!(authority, "[::1]:7687");
    }
}
