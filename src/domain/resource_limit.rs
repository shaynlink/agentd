use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceLimit {
    pub max_cpu_millis: Option<u64>,
    pub max_memory_bytes: Option<u64>,
    pub max_wall_time_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceUsage {
    pub wall_time_millis: u64,
    pub cpu_millis: Option<u64>,
    pub memory_bytes: Option<u64>,
}
