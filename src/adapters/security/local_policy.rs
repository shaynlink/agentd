use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use crate::domain::capability::{Capability, PolicyDecision};
use crate::ports::policy::{PolicyPort, RuntimeAction};

#[derive(Debug, Clone)]
pub struct LocalPolicyConfig {
    pub allow_network: bool,
    pub allowed_commands: HashSet<String>,
    pub blocked_commands: HashSet<String>,
    pub capability_effects: HashMap<Capability, bool>,
    pub allowed_exec_cwds: Vec<PathBuf>,
}

impl LocalPolicyConfig {
    pub fn read_only() -> Self {
        let mut capability_effects = HashMap::new();
        capability_effects.insert(Capability::ReadFile, true);
        capability_effects.insert(Capability::WriteFile, false);
        capability_effects.insert(Capability::DeleteFile, false);
        capability_effects.insert(Capability::ExecShell, false);
        capability_effects.insert(Capability::ExecGitRead, true);
        capability_effects.insert(Capability::ExecGitWrite, false);
        capability_effects.insert(Capability::ExecTests, false);
        capability_effects.insert(Capability::ExecNetwork, false);
        capability_effects.insert(Capability::MergeBranch, false);
        capability_effects.insert(Capability::ModifyConfig, false);

        Self {
            allow_network: false,
            allowed_commands: HashSet::new(),
            blocked_commands: ["rm", "sudo", "ssh", "scp", "docker", "kubectl"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            capability_effects,
            allowed_exec_cwds: Vec::new(),
        }
    }

    pub fn dev_safe() -> Self {
        let mut capability_effects = HashMap::new();
        capability_effects.insert(Capability::ReadFile, true);
        capability_effects.insert(Capability::WriteFile, true);
        capability_effects.insert(Capability::DeleteFile, false);
        capability_effects.insert(Capability::ExecShell, true);
        capability_effects.insert(Capability::ExecGitRead, true);
        capability_effects.insert(Capability::ExecGitWrite, true);
        capability_effects.insert(Capability::ExecTests, true);
        capability_effects.insert(Capability::ExecNetwork, false);
        capability_effects.insert(Capability::MergeBranch, false);
        capability_effects.insert(Capability::ModifyConfig, false);

        Self {
            allow_network: false,
            allowed_commands: [
                "git", "rg", "fd", "cat", "sed", "awk", "npm", "pnpm", "cargo", "pytest",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            blocked_commands: ["rm", "sudo", "ssh", "scp", "docker", "kubectl"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            capability_effects,
            allowed_exec_cwds: Vec::new(),
        }
    }

    pub fn repo_maintainer() -> Self {
        let mut cfg = Self::dev_safe();
        cfg.capability_effects.insert(Capability::MergeBranch, true);
        cfg.capability_effects.insert(Capability::ModifyConfig, true);
        cfg
    }

    pub fn full_trusted() -> Self {
        let mut capability_effects = HashMap::new();
        for capability in [
            Capability::ReadFile,
            Capability::WriteFile,
            Capability::DeleteFile,
            Capability::ExecShell,
            Capability::ExecGitRead,
            Capability::ExecGitWrite,
            Capability::ExecTests,
            Capability::ExecNetwork,
            Capability::MergeBranch,
            Capability::ModifyConfig,
        ] {
            capability_effects.insert(capability, true);
        }

        Self {
            allow_network: true,
            allowed_commands: HashSet::new(),
            blocked_commands: HashSet::new(),
            capability_effects,
            allowed_exec_cwds: Vec::new(),
        }
    }

    pub fn from_profile(profile: &str) -> Self {
        if profile.eq_ignore_ascii_case("read-only") {
            Self::read_only()
        } else if profile.eq_ignore_ascii_case("repo-maintainer") {
            Self::repo_maintainer()
        } else if profile.eq_ignore_ascii_case("full-trusted") {
            Self::full_trusted()
        } else {
            Self::dev_safe()
        }
    }
}

pub struct LocalPolicyEngine {
    profile: String,
    config: LocalPolicyConfig,
}

impl LocalPolicyEngine {
    pub fn new(profile: &str) -> Self {
        Self {
            profile: profile.to_string(),
            config: LocalPolicyConfig::from_profile(profile),
        }
    }

    pub fn with_config(profile: &str, config: LocalPolicyConfig) -> Self {
        Self {
            profile: profile.to_string(),
            config,
        }
    }
}

#[async_trait]
impl PolicyPort for LocalPolicyEngine {
    fn name(&self) -> &'static str {
        "local-policy"
    }

    async fn evaluate(&self, _session_id: &str, action: &RuntimeAction) -> Result<PolicyDecision> {
        if let Some(command) = action.command.as_ref() {
            if self.config.blocked_commands.contains(command) {
                return Ok(PolicyDecision::deny(
                    format!("command '{}' is blocked by profile {}", command, self.profile),
                    Some("blocked_commands".to_string()),
                ));
            }

            if !self.config.allowed_commands.is_empty() && !self.config.allowed_commands.contains(command)
            {
                return Ok(PolicyDecision::deny(
                    format!("command '{}' is not allowlisted", command),
                    Some("allowed_commands".to_string()),
                ));
            }
        }

        if action.capability == Capability::ExecNetwork && !self.config.allow_network {
            return Ok(PolicyDecision::deny(
                "network capability is disabled by profile",
                Some("allow_network=false".to_string()),
            ));
        }

        if !self.config.allowed_exec_cwds.is_empty()
            && !self
                .config
                .allowed_exec_cwds
                .iter()
                .any(|allowed| action.cwd.starts_with(allowed))
        {
            return Ok(PolicyDecision::deny(
                format!("cwd '{}' is not allowed", action.cwd.display()),
                Some("allowed_exec_cwds".to_string()),
            ));
        }

        let is_allowed = self
            .config
            .capability_effects
            .get(&action.capability)
            .copied()
            .unwrap_or(false);

        if is_allowed {
            Ok(PolicyDecision::allow(
                format!("capability allowed by profile {}", self.profile),
                Some("capability_effects".to_string()),
            ))
        } else {
            Ok(PolicyDecision::deny(
                format!(
                    "capability '{:?}' denied by profile {}",
                    action.capability, self.profile
                ),
                Some("capability_effects".to_string()),
            ))
        }
    }
}
