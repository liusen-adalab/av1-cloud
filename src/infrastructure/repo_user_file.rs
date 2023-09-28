use std::{borrow::Cow, time::Duration};

use crate::{
    application::file_system::video_info::{AudioInfo, MediaInfo, VideoInfo},
    domain::{
        file_system::file::{
            convert::FileNodeConverter, FileNode, FileNodeMetaData, SysFileId, UserFileId,
            VirtualPath,
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
use utils::db_pools::postgres::{pg_conn, PgConn};

use super::EffectedRow;

diesel::joinable!(user_files -> sys_files (sys_file_id));

#[derive(Queryable, Selectable, Insertable, AsChangeset, Identifiable, Debug)]
#[diesel(table_name = user_files)]
pub struct UserFilePo<'a> {
    pub id: UserFileId,
    pub sys_file_id: Option<SysFileId>,
    pub user_id: UserId,
    pub parent_id: Option<UserFileId>,
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
    pub id: SysFileId,
    pub size: i64,
    pub hash: Cow<'a, str>,
    pub path: Cow<'a, str>,
}

pub struct FileNodePo<'a> {
    pub user_file: UserFilePo<'a>,
    pub file_type: FileTypePo<'a>,
}

pub enum FileTypePo<'a> {
    File(SysFilePo<'a>),
    Video(VideoPo),
    LazyFile(SysFileId),
    Dir(Vec<FileNodePo<'a>>),
}

#[derive(From, Debug)]
pub enum PgUserFileId<'a> {
    Id(UserFileId),
    ComId((UserId, UserFileId)),
    Path(&'a VirtualPath),
}

#[derive(Debug)]
pub struct UserDirPo<'a> {
    pub file: UserFilePo<'a>,
    pub children: Vec<UserDirPo<'a>>,
}

pub async fn find_node<'a, T>(id: T, conn: &mut PgConn) -> Result<Option<FileNode>>
where
    PgUserFileId<'a>: From<T>,
{
    load_tree(id, 1, conn).await
}

async fn find_node_inner<'a, 'b, 'c, T>(
    id: T,
    conn: &'b mut PgConn,
) -> Result<Option<FileNodePo<'c>>>
where
    PgUserFileId<'a>: From<T>,
{
    macro_rules! get_result {
        ($($filter:expr),+ $(,)?) => {{
            let file = user_files::table
                    $(.filter($filter))+
                    .filter(user_files::deleted.eq(false))
                    .select(UserFilePo::as_select())
                    .for_update()
                    .get_result::<UserFilePo>(conn)
                    .await
                    .optional()?;
            let Some(file) = file else {
                return Ok(None);
            };

            let file_type = if file.is_dir {
                FileTypePo::Dir(vec![])
            } else {
                ensure!(file.sys_file_id.is_some(), "file should have sys_file_id");
                FileTypePo::LazyFile(file.sys_file_id.unwrap())
            };
            let file = FileNodePo {
                user_file: file,
                file_type,
            };

            Ok(Some(file))
        }};
    }

    match PgUserFileId::from(id) {
        PgUserFileId::Id(id) => {
            get_result!(user_files::id.eq(id))
        }
        PgUserFileId::Path(path) => {
            get_result!(
                user_files::user_id.eq(path.user_id()),
                user_files::at_dir.eq(path.parent_str()),
                user_files::file_name.eq(path.file_name()),
                user_files::is_dir.eq(true)
            )
        }
        PgUserFileId::ComId((uid, fid)) => {
            get_result!(user_files::user_id.eq(uid), user_files::id.eq(fid))
        }
    }
}

pub async fn get_filenode_data(hash: &str) -> Result<Option<FileNodeMetaData>> {
    let conn = &mut pg_conn().await?;
    let file = sys_files::table
        .filter(sys_files::hash.eq(hash))
        .select(SysFilePo::as_select())
        .for_update()
        .get_result::<SysFilePo>(conn)
        .await
        .optional()?;
    let file = file.map(FileNodeConverter::sys_file_po_to_do);
    Ok(file)
}

pub async fn save_node(node: &FileNode, conn: &mut PgConn) -> Result<EffectedRow> {
    let file_po = FileNodeConverter::do_to_po(node);
    let (u_files, s_files) = file_po.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();

    let effected = diesel::insert_into(user_files::table)
        .values(&u_files)
        .on_conflict_do_nothing()
        .execute(conn)
        .await?;

    let s_files: Vec<_> = s_files
        .into_iter()
        .filter_map(std::convert::identity)
        .collect();
    diesel::insert_into(sys_files::table)
        .values(&s_files)
        .on_conflict(sys_files::hash)
        .do_nothing()
        .execute(conn)
        .await?;

    Ok(EffectedRow {
        effected_row: effected,
        expect_row: u_files.len(),
    })
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
                user_files::at_dir.eq(path.parent_str()),
                user_files::file_name.eq(path.file_name())
            )
        }
        ExistedId::Hash(hash) => {
            pg_exist!(sys_files::table, conn, sys_files::hash.eq(hash))
        }
    }
}

pub async fn load_tree_all<'a, T>(root_id: T, conn: &mut PgConn) -> Result<Option<FileNode>>
where
    PgUserFileId<'a>: From<T>,
{
    load_tree(root_id, u32::MAX, conn).await
}

pub async fn load_tree_dep2<'a, T>(root_id: T, conn: &mut PgConn) -> Result<Option<FileNode>>
where
    PgUserFileId<'a>: From<T>,
{
    load_tree(root_id, 2, conn).await
}

pub async fn load_tree<'a, T>(root_id: T, depth: u32, conn: &mut PgConn) -> Result<Option<FileNode>>
where
    PgUserFileId<'a>: From<T>,
{
    if depth == 0 {
        return Ok(None);
    }

    let Some(root) = find_node_inner(root_id, conn).await? else {
        return Ok(None);
    };

    if !root.user_file.is_dir {
        ensure!(
            root.user_file.sys_file_id.is_some(),
            "file must have sys_file_id"
        );
        let node = FileNodePo {
            file_type: FileTypePo::LazyFile(root.user_file.sys_file_id.unwrap()),
            user_file: root.user_file,
        };
        let node = FileNodeConverter::po_to_do(node)?;
        return Ok(Some(node));
    }

    let mut children = vec![];
    load_tree_recursive(root.user_file.id, depth - 1, false, &mut children, conn).await?;

    let root = FileNodePo {
        user_file: root.user_file,
        file_type: FileTypePo::Dir(children),
    };
    let root = FileNodeConverter::po_to_do(root)?;
    Ok(Some(root))
}

pub async fn load_tree_struct<'a, T>(root_id: T) -> Result<Option<FileNode>>
where
    PgUserFileId<'a>: From<T>,
{
    let mut conn = pg_conn().await?;
    let Some(root) = find_node_inner(root_id, &mut conn).await? else {
        return Ok(None);
    };
    // release for_update lock
    drop(conn);

    ensure!(root.user_file.is_dir, "root should be dir");

    let mut children = vec![];
    let mut conn = pg_conn().await?;
    load_tree_recursive(root.user_file.id, u32::MAX, true, &mut children, &mut conn).await?;
    let root = FileNodePo {
        user_file: root.user_file,
        file_type: FileTypePo::Dir(children),
    };
    let root = FileNodeConverter::po_to_do(root)?;
    Ok(Some(root))
}

#[async_recursion::async_recursion]
async fn load_tree_recursive(
    parent_id: UserFileId,
    depth: u32,
    only_dir: bool,
    p_children: &mut Vec<FileNodePo<'_>>,
    conn: &mut PgConn,
) -> Result<()> {
    if depth == 0 {
        return Ok(());
    }

    let sql = user_files::table
        .select(UserFilePo::as_select())
        .filter(user_files::deleted.eq(false))
        .filter(user_files::parent_id.eq(parent_id));
    let children: Vec<UserFilePo> = if only_dir {
        sql.filter(user_files::is_dir.eq(true)).load(conn).await?
    } else {
        sql.load(conn).await?
    };

    for child in children {
        if child.is_dir {
            let mut ch = vec![];

            load_tree_recursive(child.id, depth - 1, only_dir, &mut ch, conn).await?;

            let d = FileNodePo {
                user_file: child,
                file_type: FileTypePo::Dir(ch),
            };
            p_children.push(d);
        } else {
            ensure!(child.sys_file_id.is_some(), "file must have sys_file_id");
            p_children.push(FileNodePo {
                file_type: FileTypePo::LazyFile(child.sys_file_id.unwrap()),
                user_file: child,
            })
        }
    }
    Ok(())
}

pub(crate) async fn update(node: &FileNode, conn: &mut PgConn) -> Result<EffectedRow> {
    let file_po = FileNodeConverter::do_to_po(node);
    let (u_files, s_files) = file_po.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();

    let s_files: Vec<_> = s_files
        .into_iter()
        .filter_map(std::convert::identity)
        .collect();
    ensure!(s_files.is_empty(), "sys_files should not be updated");

    let mut effected_total = 0;
    for u_file in &u_files {
        let effected = diesel::update(user_files::table)
            .set(u_file)
            .filter(user_files::id.eq(u_file.id))
            .execute(conn)
            .await?;
        effected_total += effected;
    }

    Ok(EffectedRow {
        effected_row: effected_total,
        expect_row: u_files.len(),
    })
}

pub async fn update_file_matedata(
    file_id: SysFileId,
    video_parsed: Option<MediaInfo>,
) -> Result<()> {
    let Some(mut video_parsed) = video_parsed else {
        let conn = &mut pg_conn().await?;
        diesel::update(dsl::sys_files)
            .filter(dsl::id.eq(file_id))
            .set((
                dsl::is_video.eq(false),
                dsl::can_be_encode.eq(false),
            ))
            .execute(conn);
        return Ok(());
    };

    video_parsed.video.durationMs = video_parsed
        .video
        .Duration
        .map(|d| Duration::from_secs_f64(d).as_millis() as u32);

    let g_bytes = serde_json::to_string(&video_parsed.general).unwrap();
    let v_bytes = serde_json::to_string(&video_parsed.video).unwrap();
    let a_bytes = video_parsed
        .audio
        .as_ref()
        .map(|a| serde_json::to_string(a).unwrap());

    let bit_rate = video_parsed.video.BitRate.map(|b| b as i32);
    let duration_ms = video_parsed.video.durationMs.map(|b| b as i32);

    let frame_count = video_parsed.video.FrameCount.map(|i| i as i32);
    let width = video_parsed.video.Width.map(|i| i as i32);
    let height = video_parsed.video.Height.map(|i| i as i32);

    let format = &video_parsed.video.Format;
    let is_format_unsupport = format.is_none();
    let can_be_encode =
        frame_count.is_some() && width.is_some() && height.is_some() && !is_format_unsupport;

    use sys_files::dsl;

    let conn = &mut pg_conn().await?;
    diesel::update(dsl::sys_files)
        .filter(dsl::id.eq(file_id))
        .set((
            dsl::general_info.eq(g_bytes),
            dsl::video_info.eq(v_bytes),
            dsl::audio_info.eq(a_bytes),
            dsl::is_video.eq(true),
            dsl::bit_rate.eq(bit_rate),
            dsl::duration_ms.eq(duration_ms),
            dsl::can_be_encode.eq(can_be_encode),
            dsl::width.eq(width),
            dsl::height.eq(height),
        ))
        .execute(conn)
        .await?;
    Ok(())
}

pub(crate) async fn get_hash(id: UserFileId) -> Result<Option<String>> {
    let conn = &mut pg_conn().await?;
    let hash = user_files::table
        .inner_join(sys_files::table)
        .filter(user_files::id.eq(id))
        .select(sys_files::hash)
        .get_result::<String>(conn)
        .await
        .optional()?;

    Ok(hash)
}

#[derive(Queryable, Selectable, Identifiable, Debug)]
#[diesel(table_name = sys_files)]
struct VideoPoInner {
    id: SysFileId,
    hash: String,
    path: String,
    size: i64,
    is_video: Option<bool>,
    transcode_from: Option<SysFileId>,
    bit_rate: Option<i32>,
    duration_ms: Option<i32>,
    height: Option<i32>,
    width: Option<i32>,
    video_info: Option<String>,
    audio_info: Option<String>,
}

pub struct VideoPo {
    pub id: SysFileId,
    pub hash: String,
    pub path: String,
    pub size: i64,

    pub is_video: Option<bool>,
    pub transcode_from: Option<SysFileId>,
    pub bit_rate: Option<i32>,
    pub duration_ms: Option<i32>,
    pub height: Option<i32>,
    pub width: Option<i32>,
    pub is_h264: bool,
    pub video_info: Option<VideoInfo>,
    pub audio_info: Option<AudioInfo>,
}

impl VideoPo {
    fn try_from_raw(video: VideoPoInner) -> Result<Self> {
        let video_info = video
            .video_info
            .map(|s| serde_json::from_str::<VideoInfo>(&s))
            .transpose()?;
        let is_h264 = video_info
            .as_ref()
            .and_then(|v| v.Format.as_ref())
            .is_some_and(|format| format.eq_ignore_ascii_case("avc"));

        let audio_info = video
            .audio_info
            .map(|s| serde_json::from_str(&s))
            .transpose()?;

        Ok(Self {
            id: video.id,
            hash: video.hash,
            path: video.path,
            size: video.size,
            is_video: video.is_video,
            transcode_from: video.transcode_from,
            bit_rate: video.bit_rate,
            duration_ms: video.duration_ms,
            height: video.height,
            width: video.width,
            video_info,
            audio_info,
            is_h264,
        })
    }
}

pub(crate) async fn find_video(id: UserFileId) -> Result<Option<FileNode>> {
    let conn = &mut pg_conn().await?;
    let res: Option<(UserFilePo, VideoPoInner)> = user_files::table
        .inner_join(sys_files::table)
        .filter(user_files::id.eq(id))
        .select((UserFilePo::as_select(), VideoPoInner::as_select()))
        .get_result::<(UserFilePo, VideoPoInner)>(conn)
        .await
        .optional()?;
    let res = res
        .map(|(user_file, video)| {
            let video = VideoPo::try_from_raw(video)?;
            let file_type = FileTypePo::Video(video);
            anyhow::Ok(FileNodePo {
                user_file,
                file_type,
            })
        })
        .transpose()?;

    let res = res.map(FileNodeConverter::po_to_do).transpose()?;
    Ok(res)
}
