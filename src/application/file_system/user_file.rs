use crate::{
    biz_ok,
    cqrs::user::UserId,
    domain::file_system::{
        file::{UserDir, UserFileId},
        service::{self},
    },
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::{
        file_sys,
        repo_user_file::{self},
    },
    pg_tx,
};
use anyhow::{ensure, Result};
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
    pub id: i64,
    pub name: String,
    pub children: Vec<DirTree>,
}

impl DirTree {
    fn from_do(tree: &UserDir) -> Self {
        let children = tree
            .dirs()
            .iter()
            .map(|child| Self::from_do(child))
            .collect();
        Self {
            id: *tree.id(),
            name: tree.path().file_name().to_string(),
            children,
        }
    }
}

pub async fn load_home(user_id: UserId) -> Result<DirTree> {
    let tree = repo_user_file::load_dir_struct(user_id).await?;
    let tree = match tree {
        Some(tree) => tree,
        None => {
            debug!("create user home");
            let tree = service::create_user_home(user_id).await?;
            let conn = &mut pg_conn().await?;
            let effted = repo_user_file::save_tree(&tree, conn).await?.is_effected();
            ensure!(effted, "save tree failed");

            for path in tree.all_paths() {
                file_sys::create_dir(path).await?;
            }
            tree
        }
    };
    Ok(DirTree::from_do(&tree))
}

#[derive(From)]
pub enum CreateDirErr {
    Create(service::CreateDirErr),
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
    let parent = ensure_exist!(
        repo_user_file::find_dir_shallow(dir_id, conn).await?,
        CreateDirErr::NoParent
    );
    ensure_biz!(*parent.user_id() == user_id, CreateDirErr::NotAllowed);
    let dir = ensure_biz!(service::create_dir(&parent, name));
    ensure_biz!(
        repo_user_file::save_tree(&dir, conn).await?.is_effected(),
        CreateDirErr::AlreadyExist
    );

    file_sys::create_dir(dir.path()).await?;

    biz_ok!(*dir.id())
}
