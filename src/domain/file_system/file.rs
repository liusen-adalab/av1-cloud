use std::{
    borrow::Cow,
    ffi::OsStr,
    path::{Path, PathBuf},
};

use getset::Getters;
use path_slash::PathExt;
use regex::Regex;

use crate::{
    domain::{file_system::service::path_manager, user::user::UserId},
    ensure_ok, flake_id_func,
};

pub type UserFileId = i64;
pub type SysFileId = i64;

#[derive(Getters, Debug)]
#[getset(get = "pub(crate)")]
pub struct UserFile {
    pub(super) id: UserFileId,
    pub(super) parent_id: Option<UserFileId>,
    pub(super) path: VirtualPath,
    pub(super) metadata: FileMetaData,
    pub(super) deleted: bool,
}

#[derive(Debug)]
pub struct VirtualPath {
    user_id: UserId,
    path: PathBuf,
}

#[derive(Getters, Debug)]
#[getset(get = "pub(crate)")]
pub struct FileMetaData {
    pub(super) id: SysFileId,
    pub(super) size: u64,
    pub(super) hash: String,
    pub(super) path: PathBuf,
}

#[derive(Getters, Debug)]
#[getset(get = "pub(crate)")]
pub struct UserDir {
    pub(super) id: UserFileId,
    pub(super) parent_id: Option<UserFileId>,
    pub(super) user_id: UserId,
    pub(super) path: VirtualPath,
    pub(super) dirs: Vec<UserDir>,
    pub(super) files: Vec<UserFile>,
}

pub enum UserFileEntry {
    File(UserFile),
    Dir(UserDir),
}

impl UserDir {
    pub(super) fn next_id() -> UserFileId {
        UserFile::next_id()
    }

    pub fn all_paths(&self) -> Vec<&VirtualPath> {
        let mut paths = vec![&self.path];
        for dir in &self.dirs {
            paths.append(&mut dir.all_paths());
        }
        for file in &self.files {
            paths.push(&file.path);
        }
        paths
    }
}

impl UserFile {
    flake_id_func!();

    pub fn user_id(&self) -> UserId {
        self.path.user_id()
    }

    pub fn increase_file_name(&mut self) {
        let new_path = self.path.increase_file_name();
        self.path = new_path;
    }

    pub fn file_data_path(&self) -> &Path {
        &self.metadata.path
    }

    pub fn file_name(&self) -> &str {
        self.path.file_name()
    }
}

#[derive(Debug)]
pub enum VirtualPathErr {
    NotAllowed,
    TooLong,
}

impl VirtualPath {
    pub fn try_build<P>(user_id: UserId, path: P) -> Result<Self, VirtualPathErr>
    where
        PathBuf: From<P>,
    {
        #[cfg(test)]
        if true {
            return Ok(Self {
                user_id,
                path: PathBuf::from(path),
            });
        }

        let path = PathBuf::from(path);
        ensure_ok!(
            path_manager()
                .user_fix_dirs()
                .iter()
                .any(|fix| path.starts_with(fix)),
            VirtualPathErr::NotAllowed
        );
        ensure_ok!(
            path.file_name().unwrap_or_default().len() < 255,
            VirtualPathErr::TooLong
        );

        Ok(Self { user_id, path })
    }

    pub fn try_build_permissive<P>(user_id: UserId, path: P) -> Result<Self, VirtualPathErr>
    where
        PathBuf: From<P>,
    {
        #[cfg(test)]
        if true {
            return Ok(Self {
                user_id,
                path: PathBuf::from(path),
            });
        }

        let path = PathBuf::from(path);
        ensure_ok!(
            path.file_name().unwrap_or_default().len() < 255,
            VirtualPathErr::TooLong
        );

        Ok(Self { user_id, path })
    }

    pub unsafe fn root(user_id: UserId) -> Self {
        Self {
            user_id,
            path: Path::new("/").to_owned(),
        }
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> Self {
        let mut path = path.as_ref();
        if path.is_absolute() {
            path = path.strip_prefix("/").unwrap()
        }
        Self {
            user_id: self.user_id,
            path: self.path.join(path),
        }
    }

    pub(super) fn is_root(&self) -> bool {
        self.path == Path::new("/")
    }

    pub fn increase_file_name(&self) -> Self {
        let stem = self.path.file_stem().unwrap().to_str().unwrap();
        let extension = self.path.extension();
        let regex = Regex::new(r"^(.*)\((\d+)\)$").unwrap();
        let new_stem = if let Some(caps) = regex.captures(stem) {
            let num = caps.get(2).unwrap().as_str().parse::<u32>().unwrap();
            format!("{}({})", caps.get(1).unwrap().as_str(), num + 1)
        } else {
            format!("{}(1)", stem)
        };

        let mut path = self.path.clone();
        let new_file_name = if let Some(extension) = extension {
            format!("{}.{}", new_stem, extension.to_str().unwrap())
        } else {
            new_stem
        };
        path.set_file_name(new_file_name);

        Self {
            user_id: self.user_id,
            path,
        }
    }

    pub fn file_name(&self) -> &str {
        self.path
            .file_name()
            .unwrap_or_else(|| &OsStr::new("/"))
            .to_str()
            .unwrap()
    }

    pub fn parent(&self) -> Self {
        Self {
            user_id: self.user_id,
            path: self
                .path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_owned(),
        }
    }

    pub fn to_str(&self) -> Cow<str> {
        self.path.to_slash_lossy()
    }

    pub fn user_id(&self) -> i64 {
        self.user_id
    }

    pub(super) fn to_path(&self, parent: &Path) -> PathBuf {
        let path = self.path.strip_prefix("/").unwrap();
        parent.join(path)
    }
}

impl FileMetaData {
    flake_id_func!();

    pub(super) fn new(size: u64, hash: String, path: PathBuf) -> Self {
        Self {
            id: Self::next_id(),
            size,
            hash: hash.to_lowercase(),
            path,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_increase_file_name() {
        let path = VirtualPath::try_build(1, "/a/b/c.txt").unwrap();
        let new_path = path.increase_file_name();
        assert_eq!(new_path.to_str(), "/a/b/c(1).txt");

        let path = VirtualPath::try_build(1, "/a/b/c(1).txt").unwrap();
        let new_path = path.increase_file_name();
        assert_eq!(new_path.to_str(), "/a/b/c(2).txt");

        let path = VirtualPath::try_build(1, "/a/b/.txt").unwrap();
        let new_path = path.increase_file_name();
        assert_eq!(new_path.to_str(), "/a/b/.txt(1)");

        let path = VirtualPath::try_build(1, "/a/b/.txt(999)").unwrap();
        let new_path = path.increase_file_name();
        assert_eq!(new_path.to_str(), "/a/b/.txt(1000)");

        let path = VirtualPath::try_build(1, "/a/b/c(-1)").unwrap();
        let new_path = path.increase_file_name();
        assert_eq!(new_path.to_str(), "/a/b/c(-1)(1)");

        let path = VirtualPath::try_build(1, "/a/b/c(1)(1)").unwrap();
        let new_path = path.increase_file_name();
        assert_eq!(new_path.to_str(), "/a/b/c(1)(2)");
    }
}
