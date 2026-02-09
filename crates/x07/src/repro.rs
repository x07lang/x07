use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolInfo {
    pub(crate) x07_version: String,
    pub(crate) x07c_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) git_sha: Option<String>,
}

pub(crate) fn tool_info() -> ToolInfo {
    ToolInfo {
        x07_version: env!("CARGO_PKG_VERSION").to_string(),
        x07c_version: x07c::X07C_VERSION.to_string(),
        git_sha: std::env::var("X07_GIT_SHA").ok(),
    }
}
