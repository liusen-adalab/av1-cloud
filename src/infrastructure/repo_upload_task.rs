use crate::{
    domain::file_system::service_upload::{UploadTask, UploadTaskId},
    redis_conn_switch::redis_conn,
};
use anyhow::Result;
use redis::AsyncCommands;

use super::RedisKey;

pub async fn find(id: UploadTaskId) -> Result<Option<UploadTask>> {
    let key = task_key(id);
    let conn = &mut redis_conn().await?;
    let task: Option<UploadTask> = conn.get(&key).await?;
    Ok(task)
}

pub async fn save(task: &UploadTask) -> Result<()> {
    let conn = &mut redis_conn().await?;
    let key = task_key(*task.id());
    conn.set_ex(&key, task, 60 * 60 * 24).await?;
    Ok(())
}

pub(crate) async fn update(task: &UploadTask) -> Result<()> {
    if task.is_completed() {
        // set ttl for task
        let conn = &mut redis_conn().await?;
        let key = task_key(*task.id());
        conn.set_ex(&key, task, 60 * 10).await?;
    } else {
        save(task).await?;
    }
    Ok(())
}

pub(crate) async fn delete(task_id: UploadTaskId) -> Result<()> {
    let conn = &mut redis_conn().await?;
    let key = task_key(task_id);
    conn.del(&key).await?;
    Ok(())
}

fn task_key(task_id: UploadTaskId) -> String {
    let key = RedisKey::new("uploading-task");
    key.add_field(task_id.to_string()).into_inner()
}

mod impl_ {
    use redis::{FromRedisValue, RedisError, RedisWrite, ToRedisArgs};

    use crate::domain::file_system::service_upload::UploadTask;

    impl FromRedisValue for UploadTask {
        fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
            let s = String::from_redis_value(v)?;
            let task: UploadTask = serde_json::from_str(&s).map_err(|err| {
                RedisError::from((
                    redis::ErrorKind::ResponseError,
                    "Serialization Error",
                    format!("{err}"),
                ))
            })?;
            Ok(task)
        }
    }

    impl ToRedisArgs for UploadTask {
        fn write_redis_args<W>(&self, out: &mut W)
        where
            W: ?Sized + RedisWrite,
        {
            let s = serde_json::to_string(&self).unwrap();
            String::write_redis_args(&s, out)
        }
    }
}
