pub mod local_securable;

use crate::config::SandboxProviderConfig;
use crate::ports::securable::SecurablePort;

pub fn build_securable(config: &SandboxProviderConfig) -> Box<dyn SecurablePort> {
    Box::new(local_securable::LocalSecurable::new(config))
}
