use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;

use crate::{domain::file_system::service::PathManager, settings::get_settings};

pub mod service;
pub mod upload;
pub mod video_info;

#[derive(Debug, Deserialize)]
pub struct FileSystemCfg {
    pub root_dir: PathBuf,
}

pub async fn init() -> Result<()> {
    let settings = &get_settings().file_system;
    PathManager::init(settings.root_dir.to_owned())?;

    Ok(())
}
