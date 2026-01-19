use serde::{Deserialize, Serialize};

pub const ABI_MAJOR_V1: u32 = 1;

pub const BACKEND_ID_MATH: &str = "x07.math";
pub const BACKEND_ID_TIME: &str = "x07.time";
pub const BACKEND_ID_EXT_FS: &str = "x07.ext.fs";
pub const BACKEND_ID_EXT_DB_SQLITE: &str = "x07.ext.db.sqlite";
pub const BACKEND_ID_EXT_DB_PG: &str = "x07.ext.db.pg";
pub const BACKEND_ID_EXT_DB_MYSQL: &str = "x07.ext.db.mysql";
pub const BACKEND_ID_EXT_DB_REDIS: &str = "x07.ext.db.redis";
pub const BACKEND_ID_EXT_REGEX: &str = "x07.ext.regex";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRequires {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world: Option<String>,
    pub requires: Vec<NativeBackendReq>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBackendReq {
    pub backend_id: String,
    pub abi_major: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}
