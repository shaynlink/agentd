use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ReadFile,
    WriteFile,
    DeleteFile,
    ExecShell,
    ExecGitRead,
    ExecGitWrite,
    ExecTests,
    ExecNetwork,
    MergeBranch,
    ModifyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    Allow,
    Deny,
    Ask,
}

impl PolicyEffect {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDecision {
    pub effect: PolicyEffect,
    pub reason: String,
    pub matched_rule: Option<String>,
}

impl PolicyDecision {
    pub fn allow(reason: impl Into<String>, matched_rule: Option<String>) -> Self {
        Self {
            effect: PolicyEffect::Allow,
            reason: reason.into(),
            matched_rule,
        }
    }

    pub fn deny(reason: impl Into<String>, matched_rule: Option<String>) -> Self {
        Self {
            effect: PolicyEffect::Deny,
            reason: reason.into(),
            matched_rule,
        }
    }

    pub fn ask(reason: impl Into<String>, matched_rule: Option<String>) -> Self {
        Self {
            effect: PolicyEffect::Ask,
            reason: reason.into(),
            matched_rule,
        }
    }
}
