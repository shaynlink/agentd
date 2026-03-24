pub mod git_like;

use anyhow::{Result, bail};

use crate::ports::versioning::VersioningPort;

pub fn build_versioning(name: &str) -> Result<Box<dyn VersioningPort>> {
    match name {
        "git" | "git-like" => Ok(Box::new(git_like::GitLikeVersioningAdapter::new())),
        other => bail!("unknown versioning adapter: {other}. expected one of: git|git-like"),
    }
}
