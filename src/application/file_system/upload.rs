use std::collections::HashSet;

use derive_more::From;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use tracing::warn;
use utils::db_pools::postgres::pg_conn;
use utils::db_pools::postgres::PgConn;

use crate::domain::file_system::service;
use crate::pg_tx;
use crate::{
    biz_ok,
    domain::{
        file_system::{
            file::{VirtualPath, VirtualPathErr},
            service::{path_manager, PathManager},
            service_upload::{UploadTask, UploadTaskState},
        },
        user::user::UserId,
    },
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::{
        file_sys::{self, UploadFileSlice},
        repo_upload_task, repo_user_file,
    },
};

#[derive(From, Debug)]
pub enum RegisterUploadTaskErr {
    PathNotAllow(VirtualPathErr),
    NoParent,
}

#[serde_as]
#[derive(Serialize)]
pub struct RegisterUploadTaskResp {
    #[serde_as(as = "DisplayFromStr")]
    pub task_id: i64,
    pub hash_existed: bool,
    pub dst_path_existed: bool,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterUploadTaskDto {
    hash: String,
    #[serde_as(as = "DisplayFromStr")]
    parent_id: i64,
    file_name: String,
}

/// return upload-task-id
pub async fn register_upload_task(
    user_id: UserId,
    task: RegisterUploadTaskDto,
) -> BizResult<RegisterUploadTaskResp, RegisterUploadTaskErr> {
    let conn = &mut pg_conn().await?;
    // create task
    let parent = ensure_exist!(
        repo_user_file::find_dir_shallow(task.parent_id, conn).await?,
        RegisterUploadTaskErr::NoParent
    );
    let path = parent.path().join(&task.file_name).to_str().into_owned();
    let dst_path = ensure_biz!(VirtualPath::try_build(user_id, path));
    let task = UploadTask::new(
        user_id,
        task.hash,
        *parent.id(),
        dst_path.file_name().to_string(),
    );

    let conn = &mut pg_conn().await?;
    // check hash
    let hash_existed = repo_user_file::exists(&**task.hash(), conn).await?;
    // check dst_path
    let dst_path_existed = repo_user_file::exists(&dst_path, conn).await?;

    // create slice dir
    let slice_dir = path_manager().upload_slice_dir(*task.id());
    file_sys::create_dir_all(&slice_dir).await?;

    repo_upload_task::save(&task).await?;

    biz_ok!(RegisterUploadTaskResp {
        task_id: *task.id(),
        hash_existed,
        dst_path_existed,
    })
}

#[derive(Serialize)]
pub struct UploadTaskDto {
    id: i64,
    hash: String,
    file_name: String,
    uploaded_slices: HashSet<u32>,
}

impl UploadTaskDto {
    pub fn new(task: &UploadTask) -> Self {
        Self {
            id: *task.id(),
            hash: task.hash().to_string(),
            file_name: task.file_name().to_string(),
            uploaded_slices: task.uploaded_slices().clone(),
        }
    }
}

pub async fn get_upload_tasks(tasks: HashSet<i64>) -> anyhow::Result<Vec<UploadTaskDto>> {
    let mut task_dto_s = vec![];
    for task_id in tasks {
        let Some(task) = repo_upload_task::find(task_id).await? else {
            warn!(task_id, "upload task not found");
            continue;
        };
        let dto = UploadTaskDto::new(&task);
        task_dto_s.push(dto);
    }
    Ok(task_dto_s)
}

pub async fn clear_upload_tasks(tasks: Vec<i64>) -> anyhow::Result<()> {
    for task_id in tasks {
        let Some(task) = repo_upload_task::find(task_id).await? else {
            warn!(task_id, "upload task not found");
            continue;
        };
        let slice_dir = path_manager().upload_slice_dir(*task.id());
        file_sys::delete(&slice_dir).await?;

        repo_upload_task::delete(task_id).await?;
    }
    Ok(())
}

pub enum StoreSliceErr {
    NoTask,
}

pub async fn store_slice(task_id: i64, index: u32, data: &[u8]) -> BizResult<(), StoreSliceErr> {
    let mut task = ensure_exist!(
        repo_upload_task::find(task_id).await?,
        StoreSliceErr::NoTask
    );
    let dir = path_manager().upload_slice_dir(task_id);
    let slice = UploadFileSlice {
        index,
        data,
        dir: &dir,
    };
    file_sys::store_slice(slice).await?;
    task.slice_done(index);
    repo_upload_task::update(&task).await?;

    biz_ok!(())
}

#[derive(Serialize)]
pub struct UploadedUserFile {
    new_name: Option<String>,
    file_id: String,
}

#[derive(From, Debug)]
pub enum FinishUploadTaskErr {
    HashNotMatch,
    SysBusy(tokio::time::error::Elapsed),
    NoParent,
    NoSlice,
    NoTask,
}

pub async fn upload_finished(task_id: i64) -> BizResult<UploadedUserFile, FinishUploadTaskErr> {
    pg_tx!(upload_finished_tx, task_id)
}

pub async fn upload_finished_tx(
    task_id: i64,
    conn: &mut PgConn,
) -> BizResult<UploadedUserFile, FinishUploadTaskErr> {
    // TODO: get lock

    let task = ensure_exist!(
        repo_upload_task::find(task_id).await?,
        FinishUploadTaskErr::NoTask
    );

    match task.state() {
        UploadTaskState::Completed(file_id) => {
            return biz_ok!(UploadedUserFile {
                new_name: None,
                file_id: file_id.to_string()
            });
        }
        UploadTaskState::Pending => {
            // do nothing and continue
        }
    }

    dbg!(&task);
    // check parent
    let parent = ensure_exist!(
        repo_user_file::find_dir_shallow(*task.parent_dir_id(), conn).await?,
        FinishUploadTaskErr::NoParent
    );

    // generate user file
    let file_data = if let Some(file) = repo_user_file::get_file_data(task.hash()).await? {
        file
    } else {
        // generate sys file
        let slice_dir = path_manager().upload_slice_dir(*task.id());
        let merged = ensure_exist!(
            file_sys::merge_slices(&slice_dir).await?,
            FinishUploadTaskErr::NoSlice
        );
        println!("hash = {}", merged.hash);
        // check hash
        ensure_biz!(
            &merged.hash == task.hash(),
            FinishUploadTaskErr::HashNotMatch
        );
        let file = PathManager::new_sys_file(merged.size, merged.hash.clone());
        dbg!(file.path());
        file_sys::create_dir_all(&file.path().parent().unwrap()).await?;
        merged.persist(file.path()).await?;
        file
    };
    let mut file = service::create_file(&parent, &task.file_name(), file_data);

    // save user file
    let mut new_name = None;
    loop {
        let effected = repo_user_file::save(&file, conn).await?;
        if effected.is_effected() {
            break;
        }
        file.increase_file_name();
        new_name = Some(file.file_name().to_string());
    }

    // clear
    let mut task = task;
    task.finished(*file.id());
    repo_upload_task::update(&task).await?;

    // do file-system stuff
    // file_sys::create_user_link(file.file_data_path().unwrap(), file.path()).await?;

    biz_ok!(UploadedUserFile {
        new_name,
        file_id: file.id().to_string()
    })
}
