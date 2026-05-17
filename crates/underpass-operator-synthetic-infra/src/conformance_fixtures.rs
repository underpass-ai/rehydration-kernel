use serde::Deserialize;
use serde_json::Value;
use underpass_operator_shared_domain::ActionArguments;
use underpass_operator_shared_infra::InfraResult;
use underpass_operator_synthetic_domain::{
    ReadApiMcpTopic, ReadApiMcpVariant, WriterExecTopic, WriterPreReadTopic, WriterPreReadVariant,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriterPreReadTopicDto {
    pub slug: String,
    pub title: String,
    pub question_hint: String,
    pub answer_hint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriterPreReadVariantDto {
    pub slug: String,
    pub question_primary_role: String,
    pub answer_secondary_role: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriterExecTopicDto {
    pub slug: String,
    pub title: String,
    pub signal: String,
    pub decision: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadApiMcpTopicDto {
    pub slug: String,
    pub title: String,
    pub agent: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadApiMcpVariantDto {
    pub slug: String,
    pub intent: String,
}

pub struct ConformanceFixtureMapper;

impl ConformanceFixtureMapper {
    pub fn writer_pre_read_topic_from_dto(
        dto: WriterPreReadTopicDto,
    ) -> InfraResult<WriterPreReadTopic> {
        Ok(WriterPreReadTopic::new(
            dto.slug,
            dto.title,
            dto.question_hint,
            dto.answer_hint,
        )?)
    }

    pub fn writer_pre_read_variant_from_dto(
        dto: WriterPreReadVariantDto,
    ) -> InfraResult<WriterPreReadVariant> {
        Ok(WriterPreReadVariant::new(
            dto.slug,
            dto.question_primary_role,
            dto.answer_secondary_role,
            dto.reason,
        )?)
    }

    pub fn writer_exec_topic_from_dto(dto: WriterExecTopicDto) -> InfraResult<WriterExecTopic> {
        Ok(WriterExecTopic::new(
            dto.slug,
            dto.title,
            dto.signal,
            dto.decision,
        )?)
    }

    pub fn read_api_mcp_topic_from_dto(dto: ReadApiMcpTopicDto) -> InfraResult<ReadApiMcpTopic> {
        Ok(ReadApiMcpTopic::new(dto.slug, dto.title, dto.agent)?)
    }

    pub fn read_api_mcp_variant_from_dto(
        dto: ReadApiMcpVariantDto,
    ) -> InfraResult<ReadApiMcpVariant> {
        Ok(ReadApiMcpVariant::new(dto.slug, dto.intent)?)
    }

    pub fn action_arguments_from_value(value: Value) -> InfraResult<ActionArguments> {
        Ok(ActionArguments::parse(value)?)
    }
}

pub fn writer_pre_read_topics_from_json(raw: &str) -> InfraResult<Vec<WriterPreReadTopic>> {
    serde_json::from_str::<Vec<WriterPreReadTopicDto>>(raw)?
        .into_iter()
        .map(ConformanceFixtureMapper::writer_pre_read_topic_from_dto)
        .collect()
}

pub fn writer_pre_read_variants_from_json(raw: &str) -> InfraResult<Vec<WriterPreReadVariant>> {
    serde_json::from_str::<Vec<WriterPreReadVariantDto>>(raw)?
        .into_iter()
        .map(ConformanceFixtureMapper::writer_pre_read_variant_from_dto)
        .collect()
}

pub fn writer_exec_topics_from_json(raw: &str) -> InfraResult<Vec<WriterExecTopic>> {
    serde_json::from_str::<Vec<WriterExecTopicDto>>(raw)?
        .into_iter()
        .map(ConformanceFixtureMapper::writer_exec_topic_from_dto)
        .collect()
}

pub fn read_api_mcp_topics_from_json(raw: &str) -> InfraResult<Vec<ReadApiMcpTopic>> {
    serde_json::from_str::<Vec<ReadApiMcpTopicDto>>(raw)?
        .into_iter()
        .map(ConformanceFixtureMapper::read_api_mcp_topic_from_dto)
        .collect()
}

pub fn read_api_mcp_variants_from_json(raw: &str) -> InfraResult<Vec<ReadApiMcpVariant>> {
    serde_json::from_str::<Vec<ReadApiMcpVariantDto>>(raw)?
        .into_iter()
        .map(ConformanceFixtureMapper::read_api_mcp_variant_from_dto)
        .collect()
}

pub fn action_arguments_from_json(raw: &str) -> InfraResult<ActionArguments> {
    let value = serde_json::from_str::<Value>(raw)?;
    ConformanceFixtureMapper::action_arguments_from_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_writer_pre_read_topic_dto_to_domain_vo() {
        let topics = writer_pre_read_topics_from_json(
            r#"[{
                "slug": "auth-refresh-race",
                "title": "mobile login refresh race",
                "question_hint": "question_refines_previous_answer",
                "answer_hint": "answer_addresses_same_question"
            }]"#,
        )
        .expect("fixture maps");

        assert_eq!(topics[0].slug().as_str(), "auth-refresh-race");
        assert_eq!(
            topics[0].question_hint().as_str(),
            "question_refines_previous_answer"
        );
    }

    #[test]
    fn rejects_empty_fixture_fields_before_generation() {
        let error = writer_pre_read_variants_from_json(
            r#"[{
                "slug": "",
                "question_primary_role": "previous_subtask_answer",
                "answer_secondary_role": "same_subtask_question",
                "reason": "previous evidence is visible"
            }]"#,
        )
        .expect_err("empty slug must fail");

        assert!(
            error
                .to_string()
                .contains("writer_variant.slug must not be empty")
        );
    }

    #[test]
    fn maps_json_payload_fixture_to_action_arguments_vo() {
        let arguments =
            action_arguments_from_json(r#"{ "ref": "node-1" }"#).expect("argument object maps");

        assert_eq!(arguments.as_value()["ref"], "node-1");
    }

    #[test]
    fn maps_read_api_mcp_topic_and_variant_to_domain_vo() {
        let topics = read_api_mcp_topics_from_json(
            r#"[{
                "slug": "auth-refresh",
                "title": "login refresh race",
                "agent": "auth"
            }]"#,
        )
        .expect("topic maps");
        let variants = read_api_mcp_variants_from_json(
            r#"[{
                "slug": "steady",
                "intent": "single current-about path"
            }]"#,
        )
        .expect("variant maps");

        let dimension = topics[0]
            .agent_dimension_for(&variants[0])
            .expect("agent dimension maps");

        assert_eq!(topics[0].slug().as_str(), "auth-refresh");
        assert_eq!(variants[0].intent().as_str(), "single current-about path");
        assert_eq!(dimension.as_str(), "agent:auth-steady");
    }
}
