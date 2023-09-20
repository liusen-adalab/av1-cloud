use std::collections::HashSet;

use anyhow::Context;
use derive_more::From;
use serde::{Deserialize, Serialize};
use tracing::warn;
use utils::db_pools::postgres::pg_conn;
use utils::db_pools::postgres::PgConn;
use utils::log_if_err;

use crate::domain::file_system::file::FileNodeMetaData;
use crate::domain::file_system::file::FileOperateErr;
use crate::domain::file_system::file::UserFileId;
use crate::domain::file_system::service_upload;
use crate::domain::file_system::service_upload::UploadTaskId;
use crate::infrastructure::av1_factory;
use crate::pg_tx;
use crate::{
    biz_ok,
    domain::{
        file_system::{
            service::path_manager,
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
    Create(service_upload::CreateTaskErr),
    NoParent,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterUploadTaskResp {
    pub task_id: UploadTaskId,
    pub hash_existed: bool,
    pub dst_path_existed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterUploadTaskDto {
    hash: String,
    parent_id: UserFileId,
    file_name: String,
}

/// return upload-task-id
pub async fn register_upload_task(
    user_id: UserId,
    task: RegisterUploadTaskDto,
) -> BizResult<RegisterUploadTaskResp, RegisterUploadTaskErr> {
    use RegisterUploadTaskErr::*;

    let conn = &mut pg_conn().await?;
    // create task
    let parent = ensure_exist!(
        repo_user_file::find_node(task.parent_id, conn).await?,
        NoParent
    );
    ensure_biz!(*parent.user_id() == user_id, NoParent);

    let task = ensure_biz!(service_upload::create_task(
        &parent,
        &task.file_name,
        task.hash
    ));

    let conn = &mut pg_conn().await?;
    // check hash
    let hash_existed = repo_user_file::exists(&**task.hash(), conn).await?;
    // check dst_path
    let dst_path_existed = repo_user_file::exists(task.path(), conn).await?;

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
#[serde(rename_all = "camelCase")]
pub struct UploadTaskDto {
    id: UploadTaskId,
    hash: String,
    file_name: String,
    uploaded_slices: HashSet<u32>,
    dst_path: String,
}

impl UploadTaskDto {
    pub fn new(task: &UploadTask) -> Self {
        Self {
            id: *task.id(),
            hash: task.hash().to_string(),
            file_name: task.path().file_name().to_string(),
            uploaded_slices: task.uploaded_slices().clone(),
            dst_path: task.path().to_str().into_owned(),
        }
    }
}

pub async fn get_upload_tasks(tasks: HashSet<UploadTaskId>) -> anyhow::Result<Vec<UploadTaskDto>> {
    let mut task_dto_s = vec![];
    for task_id in tasks {
        let Some(task) = repo_upload_task::find(task_id).await? else {
            warn!(%task_id, "upload task not found");
            continue;
        };
        let dto = UploadTaskDto::new(&task);
        task_dto_s.push(dto);
    }
    Ok(task_dto_s)
}

pub async fn clear_upload_tasks(tasks: HashSet<UploadTaskId>) -> anyhow::Result<()> {
    for task_id in tasks {
        let Some(task) = repo_upload_task::find(task_id).await? else {
            warn!(%task_id, "upload task not found");
            continue;
        };
        repo_upload_task::delete(task_id).await?;
        task_clear_bg(task);
    }
    Ok(())
}

fn task_clear_bg(task: UploadTask) {
    let clear_process = async move {
        let slice_dir = path_manager().upload_slice_dir(*task.id());
        file_sys::delete(&slice_dir).await?;

        anyhow::Ok(())
    };
    tokio::spawn(async move { log_if_err!(clear_process.await) });
}

pub enum StoreSliceErr {
    NoTask,
}

pub async fn store_slice(
    task_id: UploadTaskId,
    index: u32,
    data: &[u8],
) -> BizResult<(), StoreSliceErr> {
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
#[serde(rename_all = "camelCase")]
pub struct UploadedUserFile {
    new_name: Option<String>,
    file_id: String,
}

#[derive(From, Debug)]
pub enum FinishUploadTaskErr {
    FsDomain(FileOperateErr),
    HashNotMatch,
    NoParent,
    NoSlice,
    NoTask,
}

pub async fn upload_finished(
    task_id: UploadTaskId,
) -> BizResult<UploadedUserFile, FinishUploadTaskErr> {
    pg_tx!(upload_finished_tx, task_id)
}

pub async fn upload_finished_tx(
    task_id: UploadTaskId,
    conn: &mut PgConn,
) -> BizResult<UploadedUserFile, FinishUploadTaskErr> {
    use FinishUploadTaskErr::*;
    // TODO: get lock

    // load & check task
    let task = ensure_exist!(repo_upload_task::find(task_id).await?, NoTask);
    if let UploadTaskState::Completed(file_id) = task.state() {
        return biz_ok!(UploadedUserFile {
            new_name: None,
            file_id: file_id.to_string()
        });
    }

    // load parent
    let parent_id = (*task.user_id(), *task.parent_dir_id());
    let mut parent = ensure_exist!(
        repo_user_file::load_tree_dep2(parent_id, conn).await?,
        NoParent
    );

    // generate user file
    let file_data = ensure_biz!(load_sys_file(&task).await?);
    let sys_file_id = *file_data.id();
    let file_data_path = file_data.archived_path().clone();
    let file = ensure_biz!(parent.create_file(&task.path().file_name(), file_data));

    let new_name = file.file_name() != task.path().file_name();
    let new_name = new_name.then(|| file.file_name().to_string());
    let _effected = repo_user_file::save_node(&file, conn).await?;

    // 以下操作不能回滚，要注意顺序，以保证这个函数的幂等性

    // 为用户创建文件软链接
    file_sys::create_user_link(&file_data_path, file.path()).await?;

    // 发送信息采集的请求
    // FIXME: 为了不影响正常的流程，暂时异步请求
    tokio::spawn(async move {
        log_if_err!(av1_factory::parse_file(sys_file_id, &file_data_path)
            .await
            .context("send parse req"));
    });

    // 更新 task 状态，必须是最后一个可能失败的操作
    let mut task = task;
    task.finished(*file.id());
    repo_upload_task::update(&task).await?;

    // 确保前面的操作都成功后，异步执行清理操作
    task_clear_bg(task);

    biz_ok!(UploadedUserFile {
        new_name,
        file_id: file.id().to_string()
    })
}

async fn load_sys_file(task: &UploadTask) -> BizResult<FileNodeMetaData, FinishUploadTaskErr> {
    use FinishUploadTaskErr::*;

    if let Some(file) = repo_user_file::get_filenode_data(task.hash()).await? {
        // founded in repository
        biz_ok!(file)
    } else {
        // merge slices
        let slice_dir = path_manager().upload_slice_dir(*task.id());
        let merged = ensure_exist!(file_sys::merge_slices(&slice_dir).await?, NoSlice);
        // check hash
        ensure_biz!(&merged.hash == task.hash(), HashNotMatch);
        // persist file
        let path = path_manager().archived_path(&merged.hash);
        let file = FileNodeMetaData::new(merged.size, merged.hash.clone(), path);
        file_sys::create_dir_all(&file.archived_path().parent().unwrap()).await?;
        merged.persist(file.archived_path()).await?;

        biz_ok!(file)
    }
}
