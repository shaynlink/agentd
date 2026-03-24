pub mod local_securable;
pub mod local_policy;
pub mod local_workspace_guard;

use crate::config::SandboxProviderConfig;
use crate::ports::securable::SecurablePort;

pub fn build_securable(config: &SandboxProviderConfig) -> Box<dyn SecurablePort> {
    Box::new(local_securable::LocalSecurable::new(config))
}
