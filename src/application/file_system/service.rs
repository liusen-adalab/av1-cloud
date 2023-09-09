use crate::{
    biz_ok,
    cqrs::user::UserId,
    domain::{
        self,
        file_system::file::{FileNode, UserFileId, VirtualPath},
    },
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::{
        file_sys,
        repo_user_file::{self},
    },
    pg_tx,
};
use anyhow::{bail, ensure, Result};
use derive_more::From;
use serde::Serialize;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use tracing::debug;
use utils::db_pools::postgres::{pg_conn, PgConn};

#[serde_as]
#[derive(Serialize)]
pub struct DirTree {
    #[serde_as(as = "DisplayFromStr")]
    pub id: UserFileId,
    pub name: String,
    pub children: Vec<DirTree>,
}

impl DirTree {
    fn from_do(tree: &FileNode) -> Result<Self> {
        let children = if let Some(children) = tree.children() {
            children
                .iter()
                .map(|c| Self::from_do(c))
                .collect::<Result<_>>()?
        } else {
            bail!("tree has no children");
        };

        Ok(Self {
            id: *tree.id(),
            name: tree.file_name().to_string(),
            children,
        })
    }
}

pub async fn load_home(user_id: UserId) -> Result<DirTree> {
    let root = VirtualPath::root(user_id);
    let tree = repo_user_file::load_tree_struct(&root).await?;

    let tree = match tree {
        Some(tree) => tree,
        None => {
            debug!("create user home");
            let tree = FileNode::user_home(user_id);
            let conn = &mut pg_conn().await?;
            let all_effected = repo_user_file::save_node(&tree, conn)
                .await?
                .is_all_effected();
            ensure!(all_effected, "save tree failed");

            for path in tree.all_paths() {
                file_sys::create_dir(path).await?;
            }
            tree
        }
    };
    Ok(DirTree::from_do(&tree)?)
}

#[derive(From)]
pub enum CreateDirErr {
    Create(domain::file_system::file::CreateChildErr),
    PathErr(crate::domain::file_system::file::VirtualPathErr),

    AlreadyExist,
    NoParent,
    NotAllowed,
}

pub async fn create_dir(
    user_id: UserId,
    dir_id: UserFileId,
    name: &str,
) -> BizResult<UserFileId, CreateDirErr> {
    pg_tx!(create_dir_tx, user_id, dir_id, name)
}

pub async fn create_dir_tx(
    user_id: UserId,
    dir_id: UserFileId,
    name: &str,
    conn: &mut PgConn,
) -> BizResult<UserFileId, CreateDirErr> {
    let mut parent = ensure_exist!(
        repo_user_file::find_node(dir_id, conn).await?,
        CreateDirErr::NoParent
    );
    ensure_biz!(*parent.user_id() == user_id, CreateDirErr::NotAllowed);
    let child = ensure_biz!(parent.create_dir(name));

    ensure_biz!(
        repo_user_file::save_node(child, conn).await?.is_effected(),
        CreateDirErr::AlreadyExist
    );

    file_sys::create_dir(child.path()).await?;

    biz_ok!(*child.id())
}
