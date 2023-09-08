use std::{borrow::Cow, path::Path};

use crate::{
    domain::file_system::file::{FileMetaData, VirtualPath},
    infrastructure::repo_user_file::{SysFilePo, UserDirPo, UserFilePo},
};
use anyhow::{bail, Result};
use tracing::error;

use self::file::{UserDir, UserFile};

pub mod file;
pub mod service;
pub mod service_upload;

pub struct UserFileConverter;

impl UserFileConverter {
    pub fn po_to_do(file: UserFilePo<'_>, sys_file: SysFilePo) -> Result<UserFile> {
        let path = v_path_from_db(&file)?;
        let sys_file = Self::po_to_do_sys_file(sys_file);

        let user_file = UserFile {
            id: file.id,
            path,
            metadata: sys_file,
            parent_id: file.parent_id,
            deleted: file.deleted,
        };
        Ok(user_file)
    }

    pub fn po_to_do_sys_file(sys_file: SysFilePo) -> FileMetaData {
        FileMetaData {
            id: sys_file.id,
            size: sys_file.size as u64,
            hash: sys_file.hash.into_owned(),
            path: Path::new(&*sys_file.path).to_path_buf(),
        }
    }

    pub fn do_to_po<'a>(file: &'a UserFile) -> (UserFilePo<'a>, SysFilePo<'a>) {
        let uf = UserFilePo {
            id: file.id,
            sys_file_id: Some(file.metadata.id),
            user_id: file.user_id(),
            parent_id: file.parent_id,
            at_dir: Cow::Owned(file.path.parent().to_str().to_string()),
            file_name: Cow::Borrowed(file.file_name()),
            is_dir: false,
            deleted: file.deleted,
        };

        let sys_file = SysFilePo {
            id: file.metadata.id,
            size: (&file.metadata).size as i64,
            hash: Cow::Borrowed(&(&file.metadata).hash),
            path: file.metadata.path.to_string_lossy(),
            is_video: false,
        };

        (uf, sys_file)
    }
}

fn v_path_from_db(po: &UserFilePo) -> Result<VirtualPath> {
    let path = Path::new(&*po.at_dir).join(&*po.file_name);
    match VirtualPath::try_build_permissive(po.user_id, path) {
        Ok(p) => Ok(p),
        Err(err) => {
            error!(?po, ?err, "db data corrupted");
            bail!("invalid path from db");
        }
    }
}

pub struct TreeConverter;

impl TreeConverter {
    pub fn po_to_do(tree: UserDirPo<'_>) -> Result<UserDir> {
        let path = v_path_from_db(&tree.file)?;
        let mut dir = UserDir {
            id: tree.file.id,
            user_id: tree.file.user_id,
            path,
            dirs: vec![],
            files: vec![],
            parent_id: tree.file.parent_id,
        };
        for child in tree.children {
            dir.dirs.push(Self::po_to_do(child)?);
        }
        Ok(dir)
    }

    pub fn do_to_po(tree: &UserDir) -> Vec<(UserFilePo, Option<SysFilePo>)> {
        let mut v = Vec::new();

        v.push((Self::dir_entry_to_po(tree), None));

        Self::do_to_po_recur(tree, &mut v);
        v
    }

    pub fn do_to_po_recur<'a>(
        tree: &'a UserDir,
        v: &mut Vec<(UserFilePo<'a>, Option<SysFilePo<'a>>)>,
    ) {
        for file in &tree.files {
            let (uf, sys_file) = UserFileConverter::do_to_po(file);
            v.push((uf, Some(sys_file)));
        }

        for dir in &tree.dirs {
            v.push((Self::dir_entry_to_po(dir), None));
            Self::do_to_po_recur(dir, v);
        }
    }

    pub fn dir_entry_to_po(dir: &UserDir) -> UserFilePo {
        UserFilePo {
            id: dir.id,
            sys_file_id: None,
            user_id: dir.user_id,
            parent_id: dir.parent_id,
            at_dir: Cow::Owned(dir.path.parent().to_str().to_string()),
            file_name: Cow::Borrowed(dir.path.file_name()),
            is_dir: true,
            deleted: false,
        }
    }
}

#[macro_export]
macro_rules! flake_id_func {
    () => {
        pub(crate) fn next_id() -> i64 {
            use flaken::Flaken;
            use std::sync::{Mutex, OnceLock};
            static USER_ID_GENERATOR: OnceLock<Mutex<Flaken>> = OnceLock::new();
            let f = USER_ID_GENERATOR.get_or_init(|| Mutex::new(Flaken::default()));
            let mut lock = f.lock().unwrap();
            lock.next() as i64
        }
    };
}
