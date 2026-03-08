#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RehydrationApplication;

impl RehydrationApplication {
    pub const fn capability_name() -> &'static str {
        "deterministic-context-rehydration"
    }
}
