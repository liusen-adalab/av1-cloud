use crate::domain::file_system::file::FileOperateErr::*;
use crate::{
    biz_ok,
    domain::{
        file_system::file::{FileNode, FileOperateErr, UserFileId, VirtualPath},
        user::user::UserId,
    },
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::{
        file_sys,
        repo_user_file::{self, load_tree, load_tree_all},
    },
    pg_tx,
};
use anyhow::{bail, ensure, Result};
use serde::Serialize;
use tracing::debug;
use utils::db_pools::postgres::{pg_conn, PgConn};

#[derive(Serialize)]
pub struct DirTree {
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

pub async fn create_dir(
    user_id: UserId,
    dir_id: UserFileId,
    name: &str,
) -> BizResult<UserFileId, FileOperateErr> {
    pg_tx!(create_dir_tx, user_id, dir_id, name)
}

pub async fn create_dir_tx(
    user_id: UserId,
    dir_id: UserFileId,
    name: &str,
    conn: &mut PgConn,
) -> BizResult<UserFileId, FileOperateErr> {
    let mut parent = ensure_exist!(
        repo_user_file::load_tree_dep2((user_id, dir_id), conn).await?,
        NoParent
    );
    let child = ensure_biz!(parent.create_dir(name));

    let _ = repo_user_file::save_node(child, conn).await?;

    file_sys::create_dir(child.path()).await?;

    biz_ok!(*child.id())
}

pub async fn delete(user_id: UserId, file_ids: Vec<UserFileId>) -> BizResult<(), FileOperateErr> {
    pg_tx!(delete_tx, user_id, file_ids)
}

pub async fn delete_tx(
    user_id: UserId,
    file_ids: Vec<UserFileId>,
    conn: &mut PgConn,
) -> BizResult<(), FileOperateErr> {
    for file_id in file_ids {
        let mut node = ensure_exist!(
            repo_user_file::load_tree_all((user_id, file_id), conn).await?,
            NotFound
        );
        ensure_biz!(node.delete());

        let effected = repo_user_file::update(&node, conn).await?.is_effected();
        ensure!(effected, "delete node failed");

        file_sys::virtual_delete(node.path()).await?;
    }

    biz_ok!(())
}

pub async fn rename(
    user_id: UserId,
    file_id: UserFileId,
    new_name: &str,
) -> BizResult<(), FileOperateErr> {
    pg_tx!(rename_tx, user_id, file_id, new_name)
}

pub async fn rename_tx(
    user_id: UserId,
    file_id: UserFileId,
    new_name: &str,
    conn: &mut PgConn,
) -> BizResult<(), FileOperateErr> {
    let mut node = ensure_exist!(
        repo_user_file::find_node((user_id, file_id), conn).await?,
        NotFound
    );
    let parent_id = ensure_exist!(node.parent_id(), NotFound);

    let parent = ensure_exist!(
        repo_user_file::load_tree((user_id, *parent_id), 2, conn).await?,
        NotFound
    );

    let old_path = node.path().clone();
    ensure_biz!(parent.rename_child(&mut node, new_name));
    let new_path = node.path().clone();

    let _ = repo_user_file::update(&node, conn).await?;

    file_sys::virtual_move(&old_path, &new_path).await?;

    biz_ok!(())
}

pub async fn move_to(
    user_id: UserId,
    file_id: Vec<UserFileId>,
    new_parent_id: UserFileId,
) -> BizResult<(), FileOperateErr> {
    pg_tx!(move_to_tx, user_id, file_id, new_parent_id)
}

pub async fn move_to_tx(
    user_id: UserId,
    file_ids: Vec<UserFileId>,
    new_parent_id: UserFileId,
    conn: &mut PgConn,
) -> BizResult<(), FileOperateErr> {
    let mut new_parent = ensure_exist!(
        load_tree((user_id, new_parent_id), 2, conn).await?,
        NotFound
    );
    for file_id in file_ids {
        let origin_node = ensure_exist!(load_tree_all((user_id, file_id), conn).await?, NotFound);
        let old_path = origin_node.path().clone();
        let moved_node = ensure_biz!(origin_node.move_to(&mut new_parent));

        let effected = repo_user_file::update(&moved_node, conn)
            .await?
            .is_all_effected();
        ensure!(effected, "move node failed");

        file_sys::virtual_move(&old_path, moved_node.path()).await?;
    }

    biz_ok!(())
}

pub async fn copy_to(
    user_id: UserId,
    file_id: Vec<UserFileId>,
    new_parent_id: UserFileId,
) -> BizResult<(), FileOperateErr> {
    pg_tx!(copy_to_tx, user_id, file_id, new_parent_id)
}

pub async fn copy_to_tx(
    user_id: UserId,
    file_ids: Vec<UserFileId>,
    new_parent_id: UserFileId,
    conn: &mut PgConn,
) -> BizResult<(), FileOperateErr> {
    let mut new_parent = ensure_exist!(
        load_tree((user_id, new_parent_id), 2, conn).await?,
        NotFound
    );

    for file_id in file_ids {
        let origin_node = ensure_exist!(load_tree_all((user_id, file_id), conn).await?, NotFound);

        let new_node = ensure_biz!(origin_node.copy_to(&mut new_parent));
        let effected = repo_user_file::save_node(&new_node, conn)
            .await?
            .is_all_effected();
        ensure!(effected, "copy node failed");

        file_sys::virtual_copy(origin_node.path(), new_node.path()).await?;
    }

    biz_ok!(())
}
