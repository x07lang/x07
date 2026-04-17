use serde::Deserialize;

pub(crate) const ARCH_TASKS_INDEX_SCHEMA_VERSION: &str = "x07.arch.tasks.index@0.1.0";
pub(crate) const ARCH_TASKS_INDEX_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-arch.tasks.index.schema.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArchTasksIndex {
    pub(crate) schema_version: String,
    #[serde(default)]
    pub(crate) tasks: Vec<ArchTasksIndexTask>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArchTasksIndexTask {
    pub(crate) id: String,
    #[serde(rename = "fn")]
    pub(crate) fn_symbol: String,
    #[serde(default)]
    pub(crate) deps: Vec<String>,
    pub(crate) policy: ArchTasksPolicy,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArchTasksPolicy {
    pub(crate) criticality: String,
    pub(crate) on_failure: String,
    #[serde(default)]
    pub(crate) retry_max: Option<u32>,
}
