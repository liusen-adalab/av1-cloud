use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::RunQueryDsl;
use utils::db_pools::postgres::pg_conn;

use crate::{
    domain::{transcode_order::TranscodeTaskId, user::user::UserId},
    schema::transcode_tasks,
};

pub struct TranscodeTask {}

impl TranscodeTask {
    pub async fn running_tasks(user_id: UserId) -> anyhow::Result<Vec<TranscodeTaskId>> {
        let conn = &mut pg_conn().await?;
        let task_ids = transcode_tasks::table
            .filter(transcode_tasks::user_id.eq(user_id))
            .filter(transcode_tasks::status.eq(0))
            .select(transcode_tasks::id)
            .get_results(conn)
            .await?;
        Ok(task_ids)
    }
}
