pub mod builtin;

use anyhow::{Result, bail};

use crate::ports::runtime::RuntimePort;

pub fn build_runtime(name: &str) -> Result<Box<dyn RuntimePort>> {
    match name {
        "process" | "builtin" => Ok(Box::new(builtin::BuiltinRuntime::new())),
        "docker" => bail!("docker runtime not implemented yet"),
        "containerd" => bail!("containerd runtime not implemented yet"),
        other => {
            bail!("unknown runtime: {other}. expected one of: builtin|process|docker|containerd")
        }
    }
}
