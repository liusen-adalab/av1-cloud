use async_graphql::{ComplexObject, Enum, SimpleObject};
use diesel::{prelude::Queryable, ExpressionMethods, QueryDsl, Selectable, SelectableHelper};
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use utils::db_pools::postgres::pg_conn;

use crate::{
    domain::{
        file_system::file::{SysFileId, UserFileId},
        user::user::UserId,
    },
    schema::{sys_files, user_files},
    LocalDataTime,
};
use async_graphql::Result;

use super::{user::User, MillionTimestamp, Paginate};

/// 用户文件节点
#[derive(SimpleObject, Debug, Queryable, Selectable)]
#[graphql(complex)]
#[diesel(table_name = user_files)]
pub struct UserFile {
    pub id: UserFileId,
    pub user_id: UserId,

    #[graphql(skip)]
    pub sys_file_id: Option<i64>,

    /// 文件文件所在的目录
    pub at_dir: String,
    /// 文件名
    pub file_name: String,

    pub is_dir: bool,
}

/// 系统文件节点
#[derive(SimpleObject, Debug, Queryable, Selectable)]
#[diesel(table_name = sys_files)]
#[graphql(complex)]
pub struct FileData {
    id: SysFileId,
    /// 文件哈希
    pub hash: String,
    /// 文件大小（byte)
    pub size: i64,
    /// 是否是视频文件
    pub is_video: Option<bool>,
    /// 转码自哪个文件
    pub transcode_from: Option<i64>,
    /// 是否可以转码
    pub can_be_encode: Option<bool>,
    /// 比特率
    pub bit_rate: Option<i32>,
    /// 时长（毫秒）
    pub duration_ms: Option<i32>,
    /// 高度
    pub height: Option<i32>,
    /// 宽度
    pub width: Option<i32>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum FileType {
    /// 未解析
    UnParsed,
    /// 视频
    Video,
    /// 普通文件
    RegularFile,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum CodecType {
    #[graphql(name = "H264")]
    H264,
    #[graphql(name = "H265")]
    H265,
    #[graphql(name = "AV1")]
    Av1,
    #[graphql(name = "VP8")]
    Vp8,
    #[graphql(name = "VP9")]
    Vp9,
    /// 未支持
    UNSUPPORTED,
}

#[ComplexObject]
impl FileData {
    /// 文件类型
    async fn file_type(&self) -> Result<FileType> {
        match self.is_video {
            Some(true) => Ok(FileType::Video),
            Some(false) => Ok(FileType::RegularFile),
            None => Ok(FileType::UnParsed),
        }
    }

    /// 视频文件通用信息
    async fn general_info(&self) -> Result<Option<serde_json::Value>> {
        self.general_info_inner().await
    }

    /// 视频信息
    async fn video_info(&self) -> Result<Option<serde_json::Value>> {
        self.video_info_inner().await
    }

    /// 音频信息
    async fn audio_info(&self) -> Result<Option<serde_json::Value>> {
        self.audio_info_inner().await
    }

    /// 视频编码类型
    async fn codec_type(&self) -> Result<Option<CodecType>> {
        Ok(self.codec_type_inner().await?)
    }
}

impl FileData {
    async fn general_info_inner(&self) -> Result<Option<serde_json::Value>> {
        let mut conn = pg_conn().await?;
        let info: Option<String> = sys_files::table
            .filter(sys_files::id.eq(self.id))
            .select(sys_files::general_info)
            .first(&mut conn)
            .await?;
        let info = info.map(|info| serde_json::from_str(&info)).transpose()?;
        Ok(info)
    }

    async fn video_info_inner(&self) -> Result<Option<serde_json::Value>> {
        let mut conn = pg_conn().await?;
        let info: Option<String> = sys_files::table
            .filter(sys_files::id.eq(self.id))
            .select(sys_files::video_info)
            .first(&mut conn)
            .await?;
        let info = info.map(|info| serde_json::from_str(&info)).transpose()?;
        Ok(info)
    }

    async fn audio_info_inner(&self) -> Result<Option<serde_json::Value>> {
        let mut conn = pg_conn().await?;
        let info: Option<String> = sys_files::table
            .filter(sys_files::id.eq(self.id))
            .select(sys_files::audio_info)
            .first(&mut conn)
            .await?;
        let info = info.map(|info| serde_json::from_str(&info)).transpose()?;
        Ok(info)
    }

    async fn codec_type_inner(&self) -> Result<Option<CodecType>> {
        #[allow(non_snake_case)]
        #[derive(Deserialize, Debug)]
        struct VideoInfo {
            #[serde(default)]
            Format: Option<String>,
        }
        let v_info = self.video_info_inner().await?;
        let v_info: Option<VideoInfo> = v_info.map(|v| serde_json::from_value(v)).transpose()?;
        let codec_type =
            v_info
                .and_then(|v| v.Format)
                .map(|format| match format.to_lowercase().as_str() {
                    "h264" | "avc" => CodecType::H264,
                    "h265" | "hevc" => CodecType::H265,
                    "av1" => CodecType::Av1,
                    "vp8" => CodecType::Vp8,
                    "vp9" => CodecType::Vp9,
                    _ => CodecType::UNSUPPORTED,
                });

        Ok(codec_type)
    }
}

#[ComplexObject]
impl UserFile {
    /// 用户文件详细信息
    async fn detail(&self) -> Result<Option<FileData>> {
        Ok(self.detail_inner().await?)
    }

    /// 视频文件是否完成前期解析和切片工作，用以判断是否可以开始对这个视频转码
    async fn pre_work_completed(&self) -> Result<bool> {
        Ok(false)
    }

    async fn owner(&self) -> Result<User> {
        Ok(User::load(self.user_id).await?)
    }

    async fn create_at(&self) -> Result<MillionTimestamp> {
        Ok(self.create_at_inner().await?)
    }

    async fn last_modified(&self) -> Result<MillionTimestamp> {
        Ok(self.last_modified_inner().await?)
    }
}

impl UserFile {
    pub async fn find(id: UserFileId) -> anyhow::Result<Option<Self>> {
        let mut conn = pg_conn().await?;
        let file = user_files::table
            .filter(user_files::id.eq(id))
            .select(UserFile::as_select())
            .first::<UserFile>(&mut conn)
            .await?;
        Ok(Some(file))
    }

    async fn detail_inner(&self) -> anyhow::Result<Option<FileData>> {
        if let Some(sys_file_id) = self.sys_file_id {
            let mut conn = pg_conn().await?;
            let file = sys_files::table
                .filter(sys_files::id.eq(sys_file_id))
                .select(FileData::as_select())
                .first::<FileData>(&mut conn)
                .await?;
            Ok(Some(file))
        } else {
            Ok(None)
        }
    }

    async fn create_at_inner(&self) -> Result<MillionTimestamp> {
        let mut conn = pg_conn().await?;

        let create_at: LocalDataTime = user_files::table
            .filter(user_files::id.eq(self.id))
            .select(user_files::create_at)
            .first(&mut conn)
            .await?;
        Ok(create_at.into())
    }

    async fn last_modified_inner(&self) -> Result<MillionTimestamp> {
        let mut conn = pg_conn().await?;

        let updated_at: LocalDataTime = user_files::table
            .filter(user_files::id.eq(self.id))
            .select(user_files::updated_at)
            .first(&mut conn)
            .await?;
        Ok(updated_at.into())
    }
}

/// 文件夹节点
#[derive(SimpleObject, Default)]
pub struct DirContent {
    total: u64,
    dirs: Vec<UserFile>,
    files: Vec<UserFile>,
}

impl DirContent {
    pub async fn load(
        user_id: UserId,
        dir_id: UserFileId,
        page: Paginate,
    ) -> anyhow::Result<Option<Self>> {
        let mut conn = pg_conn().await?;
        let Some(offset) = page.cursor() else {
            return Ok(Default::default());
        };
        let total: i64 = user_files::table
            .filter(user_files::user_id.eq(user_id))
            .filter(user_files::parent_id.eq(dir_id))
            .filter(user_files::deleted.eq(false))
            .count()
            .get_result(&mut conn)
            .await?;

        let mut dir_or_files: Vec<UserFile> = user_files::table
            .filter(user_files::user_id.eq(user_id))
            .filter(user_files::parent_id.eq(dir_id))
            .filter(user_files::deleted.eq(false))
            .select(UserFile::as_select())
            .limit(page.page_size as i64)
            .offset(offset as i64)
            .order_by(user_files::is_dir.desc())
            .load::<UserFile>(&mut conn)
            .await?;

        let first_file_idx = dir_or_files.iter().position(|f| !f.is_dir);
        let files: Vec<_> = dir_or_files
            .drain(first_file_idx.unwrap_or(dir_or_files.len())..)
            .collect();

        let mut dir = Self {
            total: total as u64,
            dirs: dir_or_files,
            files,
        };
        dir.sort_by_name();
        Ok(Some(dir))
    }

    fn sort_by_name(&mut self) {
        self.dirs.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        self.files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    }
}
