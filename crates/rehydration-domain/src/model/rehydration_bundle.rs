use crate::{BundleMetadata, CaseId, Role, RoleContextPack};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrationBundle {
    case_id: CaseId,
    pack: RoleContextPack,
    sections: Vec<String>,
    metadata: BundleMetadata,
}

impl RehydrationBundle {
    pub fn new(pack: RoleContextPack, sections: Vec<String>, metadata: BundleMetadata) -> Self {
        let case_id = pack.case_header().case_id().clone();
        Self {
            case_id,
            pack,
            sections,
            metadata,
        }
    }

    pub fn root_node_id(&self) -> &CaseId {
        &self.case_id
    }

    pub fn case_id(&self) -> &CaseId {
        &self.case_id
    }

    pub fn role(&self) -> &Role {
        self.pack.role()
    }

    pub fn pack(&self) -> &RoleContextPack {
        &self.pack
    }

    pub fn sections(&self) -> &[String] {
        &self.sections
    }

    pub fn metadata(&self) -> &BundleMetadata {
        &self.metadata
    }
}
