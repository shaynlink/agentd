use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeRole {
    Admin,
    Operator,
    Viewer,
}

impl RuntimeRole {
    pub fn from_value(value: &str) -> Self {
        if value.eq_ignore_ascii_case("admin") {
            Self::Admin
        } else if value.eq_ignore_ascii_case("viewer") {
            Self::Viewer
        } else {
            Self::Operator
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Viewer => "viewer",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSet {
    pub role: RuntimeRole,
    pub allowed_commands: Vec<String>,
    pub allowed_read_paths: Vec<String>,
    pub allowed_write_paths: Vec<String>,
}

impl PermissionSet {
    pub fn can_execute_any_command(&self) -> bool {
        self.role != RuntimeRole::Viewer
    }

    pub fn bypass_acl(&self) -> bool {
        self.role == RuntimeRole::Admin
    }
}
