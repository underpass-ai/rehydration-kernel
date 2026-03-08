use std::time::SystemTime;

use rehydration_domain::{
    BundleMetadata, CaseHeader, CaseId, PlanHeader, RehydrationBundle, Role, RoleContextPack,
    WorkItem,
};

use crate::ApplicationError;

pub struct BundleAssembler;

impl BundleAssembler {
    pub fn assemble(pack: RoleContextPack, generator_version: &str) -> RehydrationBundle {
        let sections = sections_from_pack(&pack);
        RehydrationBundle::new(pack, sections, BundleMetadata::initial(generator_version))
    }

    pub fn placeholder(
        root_node_id: &str,
        role: &str,
        generator_version: &str,
    ) -> Result<RehydrationBundle, ApplicationError> {
        let case_id = CaseId::new(root_node_id)?;
        let role = Role::new(role)?;
        let summary = format!(
            "bundle for node {} role {}",
            case_id.as_str(),
            role.as_str()
        );

        let pack = RoleContextPack::new(
            role,
            CaseHeader::new(
                case_id.clone(),
                format!("Node {}", case_id.as_str()),
                summary.clone(),
                "ACTIVE",
                SystemTime::UNIX_EPOCH,
                "rehydration-kernel",
            ),
            Some(PlanHeader::new(
                format!("plan:{}", case_id.as_str()),
                1,
                "PENDING",
                0,
                0,
            )),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            summary.clone(),
            4096,
        );

        Ok(RehydrationBundle::new(
            pack,
            vec![summary],
            BundleMetadata::initial(generator_version),
        ))
    }

    pub fn synthetic(
        root_node_id: &str,
        role: &str,
        sections: Vec<String>,
        metadata: BundleMetadata,
    ) -> Result<RehydrationBundle, ApplicationError> {
        let case_id = CaseId::new(root_node_id)?;
        let role = Role::new(role)?;
        let pack = RoleContextPack::new(
            role.clone(),
            CaseHeader::new(
                case_id.clone(),
                format!("Node {}", case_id.as_str()),
                sections.first().cloned().unwrap_or_else(|| {
                    format!(
                        "bundle for node {} role {}",
                        case_id.as_str(),
                        role.as_str()
                    )
                }),
                "ACTIVE",
                SystemTime::UNIX_EPOCH,
                "rehydration-kernel",
            ),
            Some(PlanHeader::new(
                format!("plan:{}", case_id.as_str()),
                metadata.revision,
                "PLACEHOLDER",
                sections.len() as u32,
                0,
            )),
            sections
                .iter()
                .enumerate()
                .map(|(index, section)| {
                    WorkItem::new(
                        format!("section-{index}"),
                        format!("Section {}", index + 1),
                        section.clone(),
                        role.as_str(),
                        "PHASE_BUILD",
                        "READY",
                        Vec::new(),
                        (index + 1) as u32,
                    )
                })
                .collect(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            sections.join(" "),
            4096,
        );

        Ok(RehydrationBundle::new(pack, sections, metadata))
    }
}

fn sections_from_pack(pack: &RoleContextPack) -> Vec<String> {
    let mut sections = Vec::new();

    if !pack.latest_summary().trim().is_empty() {
        sections.push(pack.latest_summary().to_string());
    }

    sections.extend(pack.work_items().iter().filter_map(|work_item| {
        let title = work_item.title().trim();
        let summary = work_item.summary().trim();
        match (title.is_empty(), summary.is_empty()) {
            (true, false) => Some(summary.to_string()),
            (false, false) => Some(format!("{title}: {summary}")),
            (false, true) => Some(title.to_string()),
            (true, true) => None,
        }
    }));

    sections.extend(pack.decisions().iter().filter_map(|decision| {
        let title = decision.title().trim();
        let rationale = decision.rationale().trim();
        match (title.is_empty(), rationale.is_empty()) {
            (true, false) => Some(format!("Decision: {rationale}")),
            (false, false) => Some(format!("Decision {title}: {rationale}")),
            (false, true) => Some(format!("Decision {title}")),
            (true, true) => None,
        }
    }));

    sections.extend(pack.impacts().iter().filter_map(|impact| {
        let title = impact.title().trim();
        if title.is_empty() {
            None
        } else {
            Some(format!("Impact {}: {}", impact.impact_type(), title))
        }
    }));

    sections.extend(pack.milestones().iter().filter_map(|milestone| {
        let description = milestone.description().trim();
        if description.is_empty() {
            None
        } else {
            Some(format!(
                "Milestone {}: {}",
                milestone.milestone_type(),
                description
            ))
        }
    }));

    if sections.is_empty() {
        let summary = pack.case_header().summary().trim();
        if !summary.is_empty() {
            sections.push(summary.to_string());
        }
    }

    sections
}

#[cfg(test)]
mod tests {
    use rehydration_domain::{
        CaseHeader, CaseId, Decision, Milestone, PlanHeader, Role, RoleContextPack, TaskImpact,
        WorkItem,
    };

    use super::BundleAssembler;

    #[test]
    fn assemble_derives_ordered_sections_from_pack() {
        let case_id = CaseId::new("case-123").expect("case id is valid");
        let role = Role::new("developer").expect("role is valid");
        let pack = RoleContextPack::new(
            role.clone(),
            CaseHeader::new(
                case_id,
                "Case 123",
                "Delivery planning",
                "ACTIVE",
                std::time::SystemTime::UNIX_EPOCH,
                "planner",
            ),
            Some(PlanHeader::new("plan-123", 1, "ACTIVE", 1, 0)),
            vec![WorkItem::new(
                "task-1",
                "Implement projection model",
                "Add structured pack support",
                role.as_str(),
                "PHASE_BUILD",
                "READY",
                Vec::new(),
                1,
            )],
            vec![Decision::new(
                "decision-1",
                "Adopt projection packs",
                "Stop reading pre-rendered bundles from infrastructure",
                "ACCEPTED",
                "platform",
                std::time::SystemTime::UNIX_EPOCH,
            )],
            Vec::new(),
            vec![TaskImpact::new(
                "decision-1",
                "task-1",
                "Transport mapping",
                "DIRECT",
            )],
            vec![Milestone::new(
                "PHASE_TRANSITIONED",
                "Moved from planning to build",
                std::time::SystemTime::UNIX_EPOCH,
                "system",
            )],
            "Projection snapshot loaded",
            4096,
        );

        let bundle = BundleAssembler::assemble(pack, "0.1.0");

        assert_eq!(bundle.sections()[0], "Projection snapshot loaded");
        assert!(bundle.sections()[1].contains("Implement projection model"));
        assert!(bundle.sections()[2].contains("Adopt projection packs"));
        assert!(bundle.sections()[3].contains("DIRECT"));
        assert!(bundle.sections()[4].contains("PHASE_TRANSITIONED"));
    }
}
