use std::borrow::Cow;

use crate::{
    domain::{
        file_system::{
            file::{FileMetaData, UserDir, UserFile, UserFileId, VirtualPath},
            TreeConverter, UserFileConverter,
        },
        user::user::UserId,
    },
    pg_exist,
    schema::{sys_files, user_files},
};
use anyhow::{ensure, Result};
use derive_more::From;
use diesel::{
    prelude::{Identifiable, Insertable, Queryable},
    result::OptionalExtension,
    AsChangeset, ExpressionMethods, QueryDsl, Selectable, SelectableHelper,
};
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use tracing::debug;
use utils::db_pools::postgres::{pg_conn, PgConn};

use super::EffectedRow;

diesel::joinable!(user_files -> sys_files (sys_file_id));

#[derive(
    Queryable, Selectable, Insertable, AsChangeset, Identifiable, Debug, Serialize, Deserialize,
)]
#[diesel(table_name = user_files)]
pub struct UserFilePo<'a> {
    pub id: i64,
    pub sys_file_id: Option<i64>,
    pub user_id: i64,
    pub parent_id: Option<i64>,
    pub at_dir: Cow<'a, str>,
    pub file_name: Cow<'a, str>,
    pub is_dir: bool,
    pub deleted: bool,
}

#[derive(
    Queryable, Selectable, Insertable, AsChangeset, Identifiable, Debug, Serialize, Deserialize,
)]
#[diesel(table_name = sys_files)]
pub struct SysFilePo<'a> {
    pub id: i64,
    pub size: i64,
    pub hash: Cow<'a, str>,
    pub path: Cow<'a, str>,
    pub is_video: bool,
}

#[derive(From, Debug)]
pub enum PgUserFileId<'a> {
    Id(UserFileId),
    Path(&'a VirtualPath),
}

pub async fn find<'a, T>(id: T, conn: &mut PgConn) -> Result<Option<UserFile>>
where
    PgUserFileId<'a>: From<T>,
{
    macro_rules! get_result {
        ($conn:expr, $($filter:expr),+ $(,)?) => {{
            let Some((file, Some(sys_data))) = user_files::table
                .left_join(sys_files::table)
                $(.filter($filter))+
                .select((UserFilePo::as_select(), Option::<SysFilePo>::as_select()))
                .get_result::<(UserFilePo, Option<SysFilePo>)>($conn)
                .await
                .optional()? else {
                    return Ok(None);
                };

             let file = UserFileConverter::po_to_do(file, sys_data)?;
            Ok(Some(file))
        }};
    }
    match PgUserFileId::from(id) {
        PgUserFileId::Id(id) => {
            get_result!(conn, user_files::id.eq(id))
        }
        PgUserFileId::Path(path) => {
            get_result!(
                conn,
                user_files::user_id.eq(path.user_id()),
                user_files::at_dir.eq(path.parent().to_str()),
                user_files::file_name.eq(path.file_name()),
            )
        }
    }
}

#[derive(Debug)]
pub struct UserDirPo<'a> {
    pub file: UserFilePo<'a>,
    pub children: Vec<UserDirPo<'a>>,
}

pub async fn find_dir_shallow<'a, T>(id: T, conn: &mut PgConn) -> Result<Option<UserDir>>
where
    PgUserFileId<'a>: From<T>,
{
    macro_rules! get_result {
        ($($filter:expr),+ $(,)?) => {{
            let Some(file) = user_files::table
                        $(.filter($filter))+
                        .select(UserFilePo::as_select())
                        .for_update()
                        .get_result::<UserFilePo>(conn)
                        .await
                        .optional()? else {
                            return Ok(None);
                        };

            let file = TreeConverter::po_to_do(UserDirPo {
                file,
                children: vec![],
            })?;
            Ok(Some(file))
        }};
    }

    match PgUserFileId::from(id) {
        PgUserFileId::Id(id) => {
            get_result!(user_files::id.eq(id), user_files::is_dir.eq(true))
        }
        PgUserFileId::Path(path) => {
            get_result!(
                user_files::user_id.eq(path.user_id()),
                user_files::at_dir.eq(path.parent().to_str()),
                user_files::file_name.eq(path.file_name()),
                user_files::is_dir.eq(true)
            )
        }
    }
}

pub async fn get_file_data(hash: &str) -> Result<Option<FileMetaData>> {
    let conn = &mut pg_conn().await?;
    let file = sys_files::table
        .filter(sys_files::hash.eq(hash))
        .select(SysFilePo::as_select())
        .for_update()
        .get_result::<SysFilePo>(conn)
        .await
        .optional()?;
    let file = file.map(UserFileConverter::po_to_do_sys_file);
    Ok(file)
}

pub async fn save(file: &UserFile, conn: &mut PgConn) -> Result<EffectedRow> {
    debug!("save file: {:?}", file);
    let file_po = UserFileConverter::do_to_po(file);
    let effected = diesel::insert_into(user_files::table)
        .values(&file_po.0)
        .execute(conn)
        .await?;

    if effected == 0 {
        return Ok(EffectedRow(0));
    }

    let effected2 = diesel::insert_into(sys_files::table)
        .values(&file_po.1)
        .execute(conn)
        .await?;
    ensure!(effected == effected2, "effected not match");

    Ok(EffectedRow(effected))
}

pub async fn save_tree(tree: &UserDir, conn: &mut PgConn) -> Result<EffectedRow> {
    let file_po = TreeConverter::do_to_po(tree);
    let (user_files, sys_files) = file_po.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();
    let sys_files: Vec<_> = sys_files
        .into_iter()
        .filter_map(std::convert::identity)
        .collect();

    let effected = diesel::insert_into(user_files::table)
        .values(&user_files)
        .execute(conn)
        .await?;
    diesel::insert_into(sys_files::table)
        .values(&sys_files)
        .execute(conn)
        .await?;

    Ok(EffectedRow(effected))
}

#[derive(From, Debug)]
pub enum ExistedId<'a> {
    Id(UserFileId),
    Path(&'a VirtualPath),
    Hash(&'a str),
}

pub async fn exists<'a, T>(id: T, conn: &mut PgConn) -> Result<bool>
where
    ExistedId<'a>: From<T>,
{
    match ExistedId::from(id) {
        ExistedId::Id(id) => {
            pg_exist!(user_files::table, conn, user_files::id.eq(id))
        }
        ExistedId::Path(path) => {
            pg_exist!(
                user_files::table,
                conn,
                user_files::user_id.eq(path.user_id()),
                user_files::at_dir.eq(path.parent().to_str()),
                user_files::file_name.eq(path.file_name())
            )
        }
        ExistedId::Hash(hash) => {
            pg_exist!(sys_files::table, conn, sys_files::hash.eq(hash))
        }
    }
}

pub async fn load_dir_struct(user_id: UserId) -> Result<Option<UserDir>> {
    let mut conn = pg_conn().await?;
    let root: Option<UserFilePo> = user_files::table
        .select(UserFilePo::as_select())
        .filter(user_files::deleted.eq(false))
        .filter(user_files::user_id.eq(user_id))
        .filter(user_files::file_name.eq("/"))
        .get_result(&mut conn)
        .await
        .optional()?;
    let Some(root) = root else {
        return Ok(None);
    };

    let mut children = vec![];
    load_struc_recursive(root.id, &mut children, &mut conn).await?;
    let root = UserDirPo {
        file: root,
        children,
    };
    let root = TreeConverter::po_to_do(root)?;
    Ok(Some(root))
}

#[async_recursion::async_recursion]
async fn load_struc_recursive(
    parent_id: i64,
    p_children: &mut Vec<UserDirPo>,
    conn: &mut PgConn,
) -> Result<()> {
    let children: Vec<UserFilePo> = user_files::table
        .select(UserFilePo::as_select())
        .filter(user_files::deleted.eq(false))
        .filter(user_files::parent_id.eq(parent_id))
        .filter(user_files::is_dir.eq(true))
        .load(conn)
        .await?;

    for child in children {
        let mut ch = vec![];
        load_struc_recursive(child.id, &mut ch, conn).await?;

        let d = UserDirPo {
            file: child,
            children: ch,
        };
        p_children.push(d);
    }
    Ok(())
}
