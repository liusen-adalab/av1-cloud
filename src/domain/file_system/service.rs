use std::{path::PathBuf, sync::OnceLock};

use anyhow::ensure;

use crate::domain::user::user::UserId;

use super::{file::VirtualPath, service_upload::UploadTaskId};

pub struct PathManager {
    #[allow(dead_code)]
    root: PathBuf,
    repo_root: PathBuf,
    uploading_dir: PathBuf,
    user_space: PathBuf,
}

static PATH_MANAGER: OnceLock<PathManager> = OnceLock::new();

pub fn path_manager() -> &'static PathManager {
    PATH_MANAGER.get().unwrap()
}

impl PathManager {
    pub fn init(root: PathBuf) -> anyhow::Result<&'static Self> {
        ensure!(root.is_absolute(), "storage root must be absolute");
        let manager = PathManager {
            repo_root: root.join("archived"),
            uploading_dir: root.join("uploading"),
            user_space: root.join("user-space"),
            root,
        };
        std::fs::create_dir_all(&manager.repo_root)?;
        std::fs::create_dir_all(&manager.uploading_dir)?;
        std::fs::create_dir_all(&manager.user_space)?;

        Ok(PATH_MANAGER.get_or_init(|| manager))
    }

    pub fn user_home(&self, user_id: UserId) -> PathBuf {
        self.user_space.join(user_id.to_string())
    }

    pub fn upload_slice_dir(&self, task_id: UploadTaskId) -> PathBuf {
        self.uploading_dir.join(task_id.to_string())
    }

    pub fn archived_dir(&self, hash: &str) -> PathBuf {
        self.repo_root.join(&hash)
    }

    pub fn archived_path(&self, hash: &str) -> PathBuf {
        self.archived_dir(hash).join("origin-file")
    }
}

impl PathManager {
    pub fn virtual_to_disk(virtual_path: &VirtualPath) -> PathBuf {
        let home = path_manager().user_home(virtual_path.user_id());
        virtual_path.to_disk_path(&home)
    }
}
