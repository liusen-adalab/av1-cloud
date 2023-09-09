use std::{
    borrow::Cow,
    ffi::OsStr,
    path::{Path, PathBuf},
};

use derive_more::From;
use derive_more::IsVariant;
use getset::Getters;
use path_slash::PathExt;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{domain::user::user::UserId, ensure_ok, id_wraper};

id_wraper!(UserFileId);
id_wraper!(SysFileId);

/// 用户文件
#[derive(Getters, Debug)]
#[getset(get = "pub(crate)")]
pub struct FileNode {
    id: UserFileId,
    parent_id: Option<UserFileId>,
    user_id: UserId,
    path: VirtualPath,
    deleted: bool,
    file_type: FileType,
}

#[derive(IsVariant, Debug)]
pub enum FileType {
    File(FileNodeMetaData),
    Dir(Vec<FileNode>),
    // 大多数时候，文件的元数据是不需要的
    LazyFile(SysFileId),
}

#[derive(Getters, Debug, Clone)]
#[getset(get = "pub(crate)")]
pub struct FileNodeMetaData {
    pub id: SysFileId,
    pub size: u64,
    pub hash: String,
    pub archived_path: PathBuf,
}

/// VirtualPath 是一个虚拟的路径，用以控制用户的文件访问权限
/// 它有如下性质：
/// - 除了 "/"，所有路径都以 "/源视频" 或 "/已转码视频" 或 "/deleted" 开头
/// - 不包含 ".." 或可以解释为当前目录的 "."
/// - 不包含多余的 "/"
///
/// "/源视频" 或 "/已转码视频" 只能从数据库中读取，没有任何公开的方法可以创建
/// "/" 可以通过 VirtualPath::root() 创建，但无法通过 join 方法产生子路径，所以只能在初始化或加载用户文件系统时使用
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VirtualPath {
    user_id: UserId,
    path: PathBuf,
}

impl FileNodeMetaData {
    pub fn new(size: u64, hash: String, archived_path: PathBuf) -> Self {
        Self {
            id: SysFileId::next_id(),
            size,
            hash,
            archived_path,
        }
    }
}

#[derive(From, Debug, PartialEq, Eq)]
pub enum CreateChildErr {
    Path(VirtualPathErr),
    IAmNotDir,
}

impl FileNode {
    pub fn user_home(user_id: UserId) -> Self {
        let mut root = Self::new_dir(user_id, VirtualPath::root(user_id));
        let mut resource = Self::new_dir(user_id, VirtualPath::resource_dir(user_id));
        resource.parent_id = Some(root.id);
        let mut encoded = Self::new_dir(user_id, VirtualPath::encode_dir(user_id));
        encoded.parent_id = Some(root.id);

        let children = root.children_mut().unwrap();
        children.push(resource);
        children.push(encoded);

        root
    }

    fn new_dir(user_id: UserId, path: VirtualPath) -> Self {
        Self {
            id: UserFileId::next_id(),
            parent_id: None,
            user_id,
            path,
            deleted: false,
            file_type: FileType::Dir(vec![]),
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(self.file_type, FileType::Dir(_))
    }

    pub fn file_name(&self) -> &str {
        self.path.file_name()
    }

    // 只能在 /源视频 或 /已转码视频 下创建文件夹
    pub fn create_dir(&mut self, name: &str) -> Result<&mut Self, CreateChildErr> {
        self.create_child(name, None)
    }

    // 只能在 /源视频 或 /已转码视频 下创建文件
    pub fn create_file(
        &mut self,
        name: &str,
        metadata: FileNodeMetaData,
    ) -> Result<&mut Self, CreateChildErr> {
        self.create_child(name, Some(metadata))
    }

    pub fn create_child(
        &mut self,
        name: &str,
        metadata: Option<FileNodeMetaData>,
    ) -> Result<&mut Self, CreateChildErr> {
        use CreateChildErr::*;

        let path = self.path.join_child(name)?;
        let file_type = if let Some(meta) = metadata {
            FileType::File(meta)
        } else {
            FileType::Dir(vec![])
        };

        let mut child = Self {
            id: UserFileId::next_id(),
            parent_id: Some(self.id),
            user_id: self.user_id,
            path,
            deleted: false,
            file_type,
        };
        if let FileType::Dir(children) = &mut self.file_type {
            loop {
                let exist = children.iter().any(|ch| ch.path == child.path);
                if exist {
                    // unwrap: 由于 child.path 是从 self.path 生成的，所以必然可以合法地增加文件名计数
                    child.path = child.path.increase_file_name().unwrap();
                } else {
                    break;
                }
            }

            children.push(child);
            Ok(children.last_mut().unwrap())
        } else {
            Err(IAmNotDir)
        }
    }

    pub fn all_paths(&self) -> Vec<&VirtualPath> {
        let mut paths = vec![&self.path];
        if let FileType::Dir(dir) = &self.file_type {
            for node in dir {
                paths.append(&mut node.all_paths());
            }
        }
        paths
    }

    pub fn rename(&mut self, new_name: VirtualPath) {
        self.path = new_name;
        if let FileType::Dir(dir) = &mut self.file_type {
            for node in dir {
                node.set_parent(&self.path);
            }
        }
    }

    fn set_parent(&mut self, parent_path: &VirtualPath) {
        self.path = parent_path.join_child(self.file_name()).unwrap();
        if let FileType::Dir(dir) = &mut self.file_type {
            for node in dir {
                node.set_parent(&self.path);
            }
        }
    }

    pub fn delete(&mut self) -> Result<(), FileDeleteErr> {
        use FileDeleteErr::*;
        ensure_ok!(!self.deleted, AlreadyDeleted);
        let Some(del_path) = self.path.to_deleted() else {
            return Err(NotAllowed);
        };

        self.deleted = true;
        self.path = del_path;

        if let FileType::Dir(dir) = &mut self.file_type {
            for node in dir {
                node.delete()?;
            }
        }

        Ok(())
    }

    pub(crate) fn children(&self) -> Option<&Vec<Self>> {
        if let FileType::Dir(dir) = &self.file_type {
            Some(dir)
        } else {
            None
        }
    }

    pub(crate) fn children_mut(&mut self) -> Option<&mut Vec<Self>> {
        if let FileType::Dir(dir) = &mut self.file_type {
            Some(dir)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct NotAllowedIncreaseFileName;

pub enum FileDeleteErr {
    NotAllowed,
    AlreadyDeleted,
}

pub enum AddNodeErr {
    IAmNotDir,
}

use derive_more::Display;
#[derive(Debug, Display, PartialEq, Eq)]
pub enum VirtualPathErr {
    NotAllowed,
    TooLong,
}
impl std::error::Error for VirtualPathErr {}

impl VirtualPath {
    const SOURCE_DIR_PATH: &'static str = "/源视频";
    const ENCODED_DIR_PATH: &'static str = "/已转码视频";
    const DELETED_DIR_PATH: &'static str = "/deleted";

    pub fn root(user_id: UserId) -> Self {
        Self {
            user_id,
            path: Path::new("/").to_owned(),
        }
    }

    pub fn resource_dir(user_id: UserId) -> Self {
        Self {
            user_id,
            path: Path::new(Self::SOURCE_DIR_PATH).to_owned(),
        }
    }

    pub fn encode_dir(user_id: UserId) -> Self {
        Self {
            user_id,
            path: Path::new(Self::ENCODED_DIR_PATH).to_owned(),
        }
    }

    fn is_fix_path(path: &Path) -> bool {
        let is_root = path == Path::new("/");
        let is_source = path == Path::new(Self::SOURCE_DIR_PATH);
        let is_encoded = path == Path::new(Self::ENCODED_DIR_PATH);
        let is_deleted = path == Path::new(Self::DELETED_DIR_PATH);
        is_root || is_source || is_encoded || is_deleted
    }

    fn allow_modified(&self) -> bool {
        !Self::is_fix_path(&self.path)
    }

    fn allow_add_child(&self) -> bool {
        let is_decendant_of_source = self.path.starts_with(Self::SOURCE_DIR_PATH);
        let is_decendant_of_encoded = self.path.starts_with(Self::ENCODED_DIR_PATH);
        is_decendant_of_source || is_decendant_of_encoded
    }
}

impl VirtualPath {
    // 只能创建 "/源视频" 或 "/已转码视频" 的子路径
    // 如："/源视频/aa" 是合法的，而 "/源视频" 无法通过这个方法创建
    pub fn build<P>(user_id: UserId, path: P) -> Result<Self, VirtualPathErr>
    where
        P: AsRef<Path>,
    {
        use VirtualPathErr::*;

        let path = path.as_ref();

        ensure_ok!(!Self::is_fix_path(path), NotAllowed);

        let decendant_of_source = path.starts_with(Self::SOURCE_DIR_PATH);
        let decendant_of_encoded = path.starts_with(Self::ENCODED_DIR_PATH);
        ensure_ok!(decendant_of_source || decendant_of_encoded, NotAllowed);

        Self::build_permissive(user_id, path)
    }

    fn build_permissive<P>(user_id: UserId, path: P) -> Result<Self, VirtualPathErr>
    where
        PathBuf: From<P>,
    {
        use VirtualPathErr::*;

        let path = PathBuf::from(path);

        // only check format
        ensure_ok!(path.file_name().unwrap_or_default().len() < 255, TooLong);

        Ok(Self { user_id, path })
    }

    pub fn to_deleted(&self) -> Option<Self> {
        if !self.allow_modified() {
            return None;
        }
        Some(Self {
            user_id: self.user_id,
            path: Path::new(Self::DELETED_DIR_PATH).join(self.path.strip_prefix("/").unwrap()),
        })
    }

    pub fn join_child(&self, name: &str) -> Result<Self, VirtualPathErr> {
        use VirtualPathErr::*;

        ensure_ok!(self.allow_add_child(), NotAllowed);

        ensure_ok!(!name.contains(".."), NotAllowed);
        ensure_ok!(!name.contains("/"), NotAllowed);

        let child = self.path.join(name);
        if child.parent() != Some(&self.path) {
            return Err(VirtualPathErr::NotAllowed);
        }

        Self::build(self.user_id, child)
    }

    // 只允许在 /源视频 或 /已转码视频 下的文件修改文件名
    pub fn increase_file_name(&self) -> Option<Self> {
        if !self.allow_modified() {
            return None;
        }

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

        Some(Self {
            user_id: self.user_id,
            path,
        })
    }

    pub fn file_name(&self) -> &str {
        self.path
            .file_name()
            .unwrap_or_else(|| &OsStr::new("/"))
            .to_str()
            .unwrap()
    }

    pub fn rename(&mut self, new_name: &str) {
        self.path.set_file_name(new_name);
    }

    pub fn parent_str(&self) -> Cow<str> {
        let Some(parent) = self.path.parent() else {
            return Cow::Borrowed("");
        };

        Cow::Owned(parent.to_string_lossy().to_string())
    }

    pub fn to_str(&self) -> Cow<str> {
        self.path.to_slash_lossy()
    }

    pub fn user_id(&self) -> i64 {
        self.user_id
    }

    pub(super) fn to_disk_path(&self, actual_root: &Path) -> PathBuf {
        let path = self.path.strip_prefix("/").unwrap();
        actual_root.join(path)
    }
}

pub mod convert {
    use std::{borrow::Cow, path::Path};

    use anyhow::bail;
    use tracing::error;

    use crate::infrastructure::repo_user_file::{FileNodePo, SysFilePo, UserFilePo};

    use super::{FileNode, FileNodeMetaData, VirtualPath};

    pub struct FileNodeConverter;

    impl FileNodeConverter {
        pub fn do_to_po(file: &FileNode) -> Vec<(UserFilePo, Option<SysFilePo>)> {
            let mut v = vec![];
            Self::do_to_po_recursive(file, &mut v);
            v
        }

        fn do_to_po_recursive<'a>(
            file: &'a FileNode,
            v: &mut Vec<(UserFilePo<'a>, Option<SysFilePo<'a>>)>,
        ) {
            let mut default = UserFilePo {
                id: file.id,
                sys_file_id: None,
                user_id: file.user_id,
                parent_id: file.parent_id,
                at_dir: file.path.parent_str(),
                file_name: Cow::Borrowed(file.path.file_name()),
                is_dir: false,
                deleted: file.deleted,
            };

            match &file.file_type {
                crate::domain::file_system::file::FileType::File(meta) => {
                    default.sys_file_id = Some(meta.id);
                    default.is_dir = false;

                    let s = SysFilePo {
                        id: meta.id,
                        size: meta.size as i64,
                        hash: Cow::Borrowed(&meta.hash),
                        path: meta.archived_path.to_string_lossy(),
                        is_video: false,
                    };

                    v.push((default, Some(s)));
                }
                crate::domain::file_system::file::FileType::LazyFile(sys_id) => {
                    default.sys_file_id = Some(*sys_id);
                    default.is_dir = false;
                    v.push((default, None))
                }
                crate::domain::file_system::file::FileType::Dir(dir) => {
                    default.is_dir = true;
                    v.push((default, None));
                    for node in dir {
                        Self::do_to_po_recursive(node, v);
                    }
                }
            }
        }

        pub fn po_to_do(po: FileNodePo) -> anyhow::Result<FileNode> {
            let FileNodePo {
                user_file,
                file_type,
            } = po;
            let path = Self::v_path_from_db(&user_file)?;
            let UserFilePo {
                id,
                user_id,
                parent_id,
                deleted,
                ..
            } = user_file;

            let file_type = match file_type {
                crate::infrastructure::repo_user_file::FileTypePo::File(meta) => {
                    let meta = crate::domain::file_system::file::FileNodeMetaData {
                        id: meta.id,
                        size: meta.size as u64,
                        hash: meta.hash.into_owned(),
                        archived_path: Path::new(&*meta.path).to_path_buf(),
                    };
                    crate::domain::file_system::file::FileType::File(meta)
                }
                crate::infrastructure::repo_user_file::FileTypePo::LazyFile(id) => {
                    crate::domain::file_system::file::FileType::LazyFile(id)
                }
                crate::infrastructure::repo_user_file::FileTypePo::Dir(dir) => {
                    let mut children = vec![];
                    for node in dir {
                        children.push(Self::po_to_do(node)?);
                    }
                    crate::domain::file_system::file::FileType::Dir(children)
                }
            };

            Ok(FileNode {
                id,
                parent_id,
                user_id,
                path,
                deleted,
                file_type,
            })
        }

        pub fn sys_file_po_to_do(po: SysFilePo) -> FileNodeMetaData {
            let SysFilePo {
                id,
                size,
                hash,
                path,
                ..
            } = po;
            FileNodeMetaData {
                id,
                size: size as u64,
                hash: hash.into_owned(),
                archived_path: Path::new(&*path).to_path_buf(),
            }
        }

        fn v_path_from_db(po: &UserFilePo) -> anyhow::Result<VirtualPath> {
            let path = Path::new(&*po.at_dir).join(&*po.file_name);
            match VirtualPath::build_permissive(po.user_id, path) {
                Ok(p) => Ok(p),
                Err(err) => {
                    error!(?po, ?err, "db data corrupted");
                    bail!("invalid path from db");
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_increase_file_name() {
        let path = VirtualPath::build(1, "/源视频").unwrap();
        assert!(path.increase_file_name().is_none());

        let path = VirtualPath::build(1, "/源视频/aa").unwrap();
        let path1 = path.increase_file_name().unwrap();
        assert_eq!(path1.to_str(), "/源视频/aa(1)");

        let path2 = path1.increase_file_name().unwrap();
        assert_eq!(path2.to_str(), "/源视频/aa(2)");

        let path = VirtualPath::build(1, "/源视频/aa(1)").unwrap();
        let path1 = path.increase_file_name().unwrap();
        assert_eq!(path1.to_str(), "/源视频/aa(2)");

        let path = VirtualPath::build(1, "/源视频/aa(1).mp4").unwrap();
        let path1 = path.increase_file_name().unwrap();
        assert_eq!(path1.to_str(), "/源视频/aa(2).mp4");

        let path = VirtualPath::build(1, "/源视频/aa(-1).mp4").unwrap();
        let path1 = path.increase_file_name().unwrap();
        assert_eq!(path1.to_str(), "/源视频/aa(-1)(1).mp4");

        let path = VirtualPath::build(1, "/源视频/aa(1)(999).mp4").unwrap();
        let path1 = path.increase_file_name().unwrap();
        assert_eq!(path1.to_str(), "/源视频/aa(1)(1000).mp4");

        let path = VirtualPath::build(1, "/源视频/.aa(1)(-999).mp4").unwrap();
        let path1 = path.increase_file_name().unwrap();
        assert_eq!(path1.to_str(), "/源视频/.aa(1)(-999)(1).mp4");
    }

    #[test]
    fn t_allow_modify() {
        let root = VirtualPath::root(1);
        assert!(!root.allow_modified());

        let illegal_path = root.join_child("a");
        assert!(illegal_path.is_err());

        let path = root.join_child("已转码视频").unwrap();
        assert!(!path.allow_modified());

        let path = root
            .join_child("已转码视频")
            .unwrap()
            .join_child("a.mp4")
            .unwrap();
        assert!(path.allow_modified());

        let path = root
            .join_child("已转码视频")
            .unwrap()
            .join_child("a.mp4")
            .unwrap()
            .join_child("b.mp4")
            .unwrap();
        assert!(path.allow_modified());

        let path = root.join_child("源视频").unwrap();
        assert!(!path.allow_modified());

        let path = root
            .join_child("源视频")
            .unwrap()
            .join_child("a.mp4")
            .unwrap();
        assert!(path.allow_modified());
    }

    #[test]
    fn t_create_child() {
        use super::CreateChildErr::*;

        let mut home = FileNode::user_home(1);
        assert_eq!(
            home.create_dir("aa").unwrap_err(),
            CreateChildErr::Path(VirtualPathErr::NotAllowed)
        );

        assert_eq!(
            home.create_dir("源视频").unwrap_err(),
            CreateChildErr::Path(VirtualPathErr::NotAllowed)
        );

        let children = home.children_mut().unwrap();
        let resource = children.get_mut(0).unwrap();

        let aa_data = FileNodeMetaData::new(1, "hash".to_string(), PathBuf::from("path"));
        let aa = resource.create_file("aa", aa_data.clone()).unwrap();
        assert_eq!(aa.create_dir("name").unwrap_err(), IAmNotDir);
        let aa1 = resource.create_file("aa", aa_data.clone()).unwrap();
        assert_eq!(aa1.path().to_str(), "/源视频/aa(1)");

        let encoded = children.get_mut(1).unwrap();
        use super::VirtualPathErr;
        assert_eq!(
            encoded.create_dir(".").unwrap_err(),
            CreateChildErr::Path(VirtualPathErr::NotAllowed)
        );
        assert_eq!(
            encoded.create_dir("./aa").unwrap_err(),
            CreateChildErr::Path(VirtualPathErr::NotAllowed)
        );
        assert_eq!(
            encoded.create_dir("..").unwrap_err(),
            CreateChildErr::Path(VirtualPathErr::NotAllowed)
        );
    }

    #[test]
    fn t_to_disk_path() {
        let user_root = Path::new("/storage/user-space/1");

        let mut home = FileNode::user_home(1);
        let resource = home.children_mut().unwrap().get_mut(0).unwrap();
        let res_path = resource.path.to_disk_path(user_root);
        assert_eq!(res_path, user_root.join("源视频"));

        resource.create_dir("name").unwrap();

        let name = resource.children_mut().unwrap().get_mut(0).unwrap();
        let path = name.path.to_disk_path(user_root);
        assert_eq!(path, res_path.join("name"));
    }
}
