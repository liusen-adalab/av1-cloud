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
#[derive(Getters, Debug, Clone)]
#[getset(get = "pub(crate)")]
pub struct FileNode {
    id: UserFileId,
    parent_id: Option<UserFileId>,
    user_id: UserId,
    path: VirtualPath,
    deleted: bool,
    file_type: FileType,
}

#[derive(IsVariant, Debug, Clone)]
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
    AlreadyExist,
}

#[derive(From, Debug, PartialEq, Eq)]
pub enum MoveFileErr {
    Path(VirtualPathErr),
    ParentNotDir,
    AlreadyExist,
}

#[derive(From, Debug, PartialEq, Eq)]
pub enum RenameFileErr {
    Path(VirtualPathErr),
    ParentNotDir,
    AlreadyExist,
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

    pub fn copy_to<'a>(&self, new_parent: &'a mut Self) -> Result<&'a mut Self, MoveFileErr> {
        let copyed = self.copy(new_parent.id);
        let copyed = copyed.move_to(new_parent)?;
        Ok(copyed)
    }

    fn copy(&self, parent_id: UserFileId) -> Self {
        let mut copyed = self.clone();
        copyed.id = UserFileId::next_id();
        copyed.parent_id = Some(parent_id);

        if let FileType::Dir(dir) = &mut copyed.file_type {
            for node in dir {
                node.copy(copyed.id);
            }
        }

        copyed
    }

    pub fn move_to<'a>(mut self, new_parent: &'a mut Self) -> Result<&'a mut Self, MoveFileErr> {
        use MoveFileErr::*;
        let FileType::Dir(children) = &mut new_parent.file_type else {
            return Err(ParentNotDir);
        };
        let new_path = new_parent.path.join_child(self.file_name())?;
        let existed = children.iter().any(|ch| ch.path == new_path);
        ensure_ok!(!existed, AlreadyExist);

        self.move_inner(new_path)?;

        self.parent_id = Some(new_parent.id);
        children.push(self);
        Ok(children.last_mut().unwrap())
    }

    pub fn rename_child(&self, child: &mut FileNode, new_name: &str) -> Result<(), RenameFileErr> {
        use RenameFileErr::*;
        let FileType::Dir(children) = &self.file_type else {
            return Err(RenameFileErr::ParentNotDir);
        };

        let new_path = child.path.rename(new_name)?;
        let existed = children.iter().any(|ch| ch.path == new_path);
        ensure_ok!(!existed, AlreadyExist);

        child.move_inner(new_path)?;

        Ok(())
    }

    fn move_inner(&mut self, new_path: VirtualPath) -> Result<(), VirtualPathErr> {
        self.path = new_path;
        if let FileType::Dir(dir) = &mut self.file_type {
            for node in dir {
                let new_path = self.path.join_child(node.file_name())?;
                node.move_inner(new_path)?;
            }
        }
        Ok(())
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

#[derive(PartialEq, Eq, Debug)]
pub enum FileDeleteErr {
    NotAllowed,
    AlreadyDeleted,
}

#[derive(Debug, From)]
pub enum RenameErr {
    Path(VirtualPathErr),
    AlreadyExist,
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
    pub fn build<Id, P>(user_id: Id, path: P) -> Result<Self, VirtualPathErr>
    where
        P: AsRef<Path>,
        UserId: From<Id>,
    {
        use VirtualPathErr::*;
        let user_id = UserId::from(user_id);

        let path = path.as_ref().to_slash_lossy();
        let path = clean_path::clean(&*path);

        ensure_ok!(!Self::is_fix_path(&path), NotAllowed);

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

    pub fn rename(&self, new_name: &str) -> Result<Self, VirtualPathErr> {
        let mut path = self.path.clone();
        path.set_file_name(new_name);

        Self::build(self.user_id, path)
    }

    pub fn parent_str(&self) -> Cow<str> {
        let Some(parent) = self.path.parent() else {
            return Cow::Borrowed("");
        };

        parent.to_slash_lossy()
    }

    pub fn to_str(&self) -> Cow<str> {
        self.path.to_slash_lossy()
    }

    pub fn user_id(&self) -> UserId {
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
        let root = VirtualPath::root(1.into());
        assert!(root.increase_file_name().is_none());

        let path = VirtualPath::build_permissive(1.into(), "/源视频").unwrap();
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
        let root = VirtualPath::root(1.into());
        assert!(!root.allow_modified());

        let illegal_path = root.join_child("a");
        assert!(illegal_path.is_err());

        assert_eq!(
            root.join_child("/源视频").unwrap_err(),
            VirtualPathErr::NotAllowed
        );
        assert_eq!(
            root.join_child("已转码视频").unwrap_err(),
            VirtualPathErr::NotAllowed
        );
        assert_eq!(
            root.join_child("aa").unwrap_err(),
            VirtualPathErr::NotAllowed
        );

        let resource = VirtualPath::build_permissive(1.into(), "/源视频").unwrap();
        let aabb = resource.join_child("aa").unwrap().join_child("bb").unwrap();
        assert!(aabb.allow_modified());
    }

    #[test]
    fn t_create_child() {
        use super::CreateChildErr::*;

        let mut home = FileNode::user_home(1.into());
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

        let mut home = FileNode::user_home(1.into());
        let resource = home.children_mut().unwrap().get_mut(0).unwrap();
        let res_path = resource.path.to_disk_path(user_root);
        assert_eq!(res_path, user_root.join("源视频"));

        resource.create_dir("name").unwrap();

        let name = resource.children_mut().unwrap().get_mut(0).unwrap();
        let path = name.path.to_disk_path(user_root);
        assert_eq!(path, res_path.join("name"));
    }

    #[test]
    fn t_rename() {
        let home = &mut FileNode::user_home(1.into());
        let (aa, _bb) = test_user_home(home);
        let mut aa = aa.clone();
        aa.create_dir("bb").unwrap();

        let resource = home.children().unwrap().get(0).unwrap();
        resource.rename_child(&mut aa, "cc").unwrap();

        let aabb = aa.children().unwrap().get(0).unwrap();
        assert_eq!(aa.path().to_str(), "/源视频/cc");
        assert_eq!(aabb.path().to_str(), "/源视频/cc/bb");
        assert_eq!(aa.id, aabb.parent_id.unwrap());
    }

    #[test]
    fn t_copy() {
        let mut home = FileNode::user_home(1.into());
        let resource = home.children_mut().unwrap().get_mut(0).unwrap();

        resource.create_dir("aa").unwrap();
        resource.create_dir("bb").unwrap();

        let ([aa], [bb]) = resource.children_mut().unwrap().split_at_mut(1) else {
            panic!()
        };

        let aacc = aa.create_dir("cc").unwrap();
        let bbcc = aacc.copy_to(bb).unwrap();
        assert_eq!(bbcc.path().to_str(), "/源视频/bb/cc");

        assert_eq!(bbcc.copy_to(aa).unwrap_err(), MoveFileErr::AlreadyExist);

        let bbccaa = aa.copy_to(bbcc).unwrap();
        assert_eq!(bbccaa.path().to_str(), "/源视频/bb/cc/aa");
    }

    // /
    // └── 源视频
    // |    ├── aa
    // |    └── bb
    // ├── 已转码视频
    //
    // return (aa, bb)
    fn test_user_home(home: &mut FileNode) -> (&mut FileNode, &mut FileNode) {
        let resource = home.children_mut().unwrap().get_mut(0).unwrap();
        resource.create_dir("aa").unwrap();
        resource.create_dir("bb").unwrap();

        let ([aa], [bb]) = resource.children_mut().unwrap().split_at_mut(1) else {
            panic!()
        };
        (aa, bb)
    }

    #[test]
    fn t_delete() {
        use FileDeleteErr::*;
        let mut home = FileNode::user_home(1.into());
        assert_eq!(home.delete().unwrap_err(), NotAllowed);

        let (aa, bb) = test_user_home(&mut home);

        aa.create_dir("cc").unwrap();
        aa.delete().unwrap();
        assert!(aa.deleted);
        assert_eq!(aa.path().to_str(), "/deleted/源视频/aa");
        let aacc = aa.children_mut().unwrap().get_mut(0).unwrap();
        assert!(aacc.deleted);
        assert_eq!(aacc.path().to_str(), "/deleted/源视频/aa/cc");

        bb.delete().unwrap();
        assert!(bb.deleted);
        assert_eq!(bb.path().to_str(), "/deleted/源视频/bb");
    }
}
