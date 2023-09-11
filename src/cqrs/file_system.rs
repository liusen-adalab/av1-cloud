use async_graphql::{ComplexObject, SimpleObject};
use diesel::{prelude::Queryable, ExpressionMethods, QueryDsl, Selectable, SelectableHelper};
use diesel_async::RunQueryDsl;
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
pub struct FileData {
    id: SysFileId,
    /// 文件哈希
    pub hash: String,
    /// 文件大小（byte)
    pub size: i64,
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

        let mut files: Vec<UserFile> = user_files::table
            .filter(user_files::user_id.eq(user_id))
            .filter(user_files::parent_id.eq(dir_id))
            .filter(user_files::deleted.eq(false))
            .select(UserFile::as_select())
            .limit(page.page_size as i64)
            .offset(offset as i64)
            .order_by(user_files::is_dir.asc())
            .load::<UserFile>(&mut conn)
            .await?;

        let idx = files.iter().position(|f| f.is_dir);
        let dirs: Vec<_> = files.drain(idx.unwrap_or(files.len())..).collect();

        Ok(Some(Self {
            total: total as u64,
            dirs,
            files,
        }))
    }
}
