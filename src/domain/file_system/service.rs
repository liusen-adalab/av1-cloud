use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::ensure;
use anyhow::Result;

use crate::{domain::user::user::UserId, ensure_ok};

use super::file::{FileMetaData, UserDir, UserFile, VirtualPath};

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

    pub fn user_fix_dirs(&self) -> [&'static Path; 2] {
        [Path::new("/源视频"), Path::new("/已转码视频")]
    }

    pub fn upload_slice_dir(&self, task_id: i64) -> PathBuf {
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
    pub fn new_sys_file(size: u64, hash: String) -> FileMetaData {
        let path = path_manager().archived_path(&hash);
        FileMetaData::new(size, hash, path)
    }

    pub fn virtual_path_to_sys(virtual_path: &VirtualPath) -> PathBuf {
        let prefix = path_manager().user_home(virtual_path.user_id());
        virtual_path.to_path(&prefix)
    }
}

pub async fn create_user_home(user_id: UserId) -> Result<UserDir> {
    let [dir1, dir2] = path_manager().user_fix_dirs();
    let mut root = UserDir {
        id: UserDir::next_id(),
        user_id,
        path: unsafe { VirtualPath::root(user_id) },
        dirs: vec![],
        files: vec![],
        parent_id: None,
    };

    let dir1 = UserDir {
        id: UserDir::next_id(),
        parent_id: Some(root.id),
        user_id,
        path: root.path.join(dir1),
        dirs: vec![],
        files: vec![],
    };
    let dir2 = UserDir {
        id: UserDir::next_id(),
        parent_id: Some(root.id),
        user_id,
        path: root.path.join(dir2),
        dirs: vec![],
        files: vec![],
    };
    root.dirs.push(dir1);
    root.dirs.push(dir2);

    Ok(root)
}

pub enum CreateDirErr {
    NotAllowedPath,
}

pub fn create_dir(parent: &UserDir, name: &str) -> Result<UserDir, CreateDirErr> {
    ensure_ok!(!parent.path.is_root(), CreateDirErr::NotAllowedPath);
    let path = parent.path.join(name);
    let dir = UserDir {
        id: UserDir::next_id(),
        user_id: parent.user_id,
        path,
        dirs: vec![],
        files: vec![],
        parent_id: Some(parent.id),
    };
    Ok(dir)
}

pub fn create_file(parent: &UserDir, name: &str, metadata: FileMetaData) -> UserFile {
    dbg!(&parent);
    let path = parent.path.join(name);
    UserFile {
        id: UserFile::next_id(),
        parent_id: Some(parent.id),
        path,
        metadata,
        deleted: false,
    }
}
