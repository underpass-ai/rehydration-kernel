use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionSelectionMode {
    All,
    Only,
    Except,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimensionSelection {
    mode: DimensionSelectionMode,
    dimensions: BTreeSet<String>,
}

impl DimensionSelection {
    pub fn all() -> Self {
        Self {
            mode: DimensionSelectionMode::All,
            dimensions: BTreeSet::new(),
        }
    }

    pub fn only(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            mode: DimensionSelectionMode::Only,
            dimensions: normalize_dimensions(values),
        }
    }

    pub fn except(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            mode: DimensionSelectionMode::Except,
            dimensions: normalize_dimensions(values),
        }
    }

    pub fn mode(&self) -> DimensionSelectionMode {
        self.mode
    }

    pub fn dimensions(&self) -> &BTreeSet<String> {
        &self.dimensions
    }

    pub fn includes(&self, dimension: &str) -> bool {
        match self.mode {
            DimensionSelectionMode::All => true,
            DimensionSelectionMode::Only => self.dimensions.contains(dimension),
            DimensionSelectionMode::Except => !self.dimensions.contains(dimension),
        }
    }
}

impl Default for DimensionSelection {
    fn default() -> Self {
        Self::all()
    }
}

fn normalize_dimensions(values: impl IntoIterator<Item = impl Into<String>>) -> BTreeSet<String> {
    values
        .into_iter()
        .map(Into::into)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::DimensionSelection;

    #[test]
    fn selection_filters_dimensions() {
        let only = DimensionSelection::only(["conversation", "entity", " "]);
        assert!(only.includes("conversation"));
        assert!(!only.includes("benchmark_record"));

        let except = DimensionSelection::except(["entity"]);
        assert!(except.includes("conversation"));
        assert!(!except.includes("entity"));
    }
}
