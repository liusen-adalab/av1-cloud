use std::collections::HashSet;

use actix_identity::Identity;
use actix_multipart::form::bytes::Bytes;
use actix_multipart::form::text::Text;
use actix_multipart::form::{MultipartForm, MultipartFormConfig};
use actix_session::SessionExt;
use actix_web::web::{self, Json, Query};
use actix_web::HttpRequest;
use serde::{Deserialize, Serialize};
use utils::code;

use crate::application::file_system::service::{self, DirTree};
use crate::application::file_system::upload::{
    self, FinishUploadTaskErr, RegisterUploadTaskDto, RegisterUploadTaskErr,
    RegisterUploadTaskResp, StoreSliceErr, UploadTaskDto, UploadedUserFile,
};
use crate::domain::file_system::file::{FileOperateErr, UserFileId, VirtualPathErr};
use crate::domain::file_system::service_upload::UploadTaskId;
use crate::domain::user::user::UserId;
use crate::http::{ApiError, ApiResponse};
use crate::{http::JsonResponse, status_doc};

code! {
    mod = "file_system";
    index = 12;
    err_trait = crate::http::HttpBizError;

    pub FileOperate = 200 {
        not_allowed = "不允许操作的文件",
        already_deleted = "文件已删除",
        not_found = "文件不存在",
        already_exist = "文件已存在",
        parent_not_found = "父文件不存在",
        parent_not_dir = "父文件不是目录",
    }

    pub PathFormat = 210 {
        not_allow = "不允许的路径格式",
        bad_file_name = "文件名不合法",
        too_long = "路径过长",
        must_absolute = "必须是绝对路径",
    }

    ---

    RegisterUploadTask {
        no_parent = "父目录不存在",
        parent_not_dir = "父级文件不是目录",
        bad_file_name = "文件名不合法",
    }

    UploadSlice {
        no_task = "任务不存在",
    }

    FinishUpload {
        no_task = "任务不存在",
        hash_not_match = "文件hash不匹配",
        sys_busy = "系统繁忙",
        no_parent = "父目录不存在",
        no_slice = "文件片段不存在",
    }
}

impl From<RegisterUploadTaskErr> for ApiError {
    fn from(value: RegisterUploadTaskErr) -> Self {
        match value {
            RegisterUploadTaskErr::NoParent => REGISTER_UPLOAD_TASK.no_parent.into(),
            RegisterUploadTaskErr::Create(c) => match c {
                crate::domain::file_system::service_upload::CreateTaskErr::ParentNotDir => {
                    REGISTER_UPLOAD_TASK.parent_not_dir.into()
                }
                crate::domain::file_system::service_upload::CreateTaskErr::BadFileName => {
                    REGISTER_UPLOAD_TASK.bad_file_name.into()
                }
            },
        }
    }
}

impl From<StoreSliceErr> for ApiError {
    fn from(value: StoreSliceErr) -> Self {
        match value {
            StoreSliceErr::NoTask => UPLOAD_SLICE.no_task.into(),
        }
    }
}

impl From<FinishUploadTaskErr> for ApiError {
    fn from(value: FinishUploadTaskErr) -> Self {
        match value {
            FinishUploadTaskErr::NoTask => FINISH_UPLOAD.no_task.into(),
            FinishUploadTaskErr::HashNotMatch => FINISH_UPLOAD.hash_not_match.into(),
            FinishUploadTaskErr::NoParent => FINISH_UPLOAD.no_parent.into(),
            FinishUploadTaskErr::NoSlice => FINISH_UPLOAD.no_slice.into(),
            FinishUploadTaskErr::FsDomain(f) => f.into(),
        }
    }
}

impl From<FileOperateErr> for ApiError {
    fn from(value: FileOperateErr) -> Self {
        match value {
            FileOperateErr::AlreadyDeleted => FILE_OPERATE.already_deleted.into(),
            FileOperateErr::NotFound => FILE_OPERATE.not_found.into(),
            FileOperateErr::AlreadyExist => FILE_OPERATE.already_exist.into(),
            FileOperateErr::ParentNotDir => FILE_OPERATE.parent_not_dir.into(),
            FileOperateErr::NoParent => FILE_OPERATE.parent_not_found.into(),
            FileOperateErr::Path(p) => p.into(),
        }
    }
}

impl From<VirtualPathErr> for ApiError {
    fn from(value: VirtualPathErr) -> Self {
        match value {
            VirtualPathErr::NotAllowed => PATH_FORMAT.not_allow.into(),
            VirtualPathErr::BadFileName => PATH_FORMAT.bad_file_name.into(),
            VirtualPathErr::TooLong => PATH_FORMAT.too_long.into(),
            VirtualPathErr::MustAbsolute => PATH_FORMAT.must_absolute.into(),
        }
    }
}

status_doc!();

pub fn actix_config(cfg: &mut web::ServiceConfig) {
    let m_limit = MultipartFormConfig::default().memory_limit(1024 * 1024 * 100);
    cfg.service(
        web::scope("/api/fs")
            .service(web::resource("/doc").route(web::get().to(biz_status_doc)))
            .service(web::resource("/home").route(web::get().to(load_home)))
            .service(web::resource("/create_dir").route(web::post().to(create_dir)))
            .service(web::resource("/delete").route(web::post().to(delete)))
            .service(web::resource("/copy").route(web::post().to(copy)))
            .service(web::resource("/move").route(web::post().to(move_to)))
            .service(web::resource("/rename").route(web::post().to(rename)))
            // upload
            .service(
                web::resource("/register_upload_task").route(web::post().to(register_upload_task)),
            )
            .service(web::resource("/del_upload_task").route(web::post().to(del_upload_task)))
            .service(
                web::resource("/upload_tasks")
                    .route(web::get().to(get_upload_tasks))
                    .route(web::delete().to(clear_upload_tasks)),
            )
            .service(
                web::resource("/upload_slice")
                    .app_data(m_limit.clone())
                    .route(web::post().to(upload_slice)),
            )
            .service(web::resource("/finish_upload").route(web::post().to(upload_finished))),
    )
    .service(
        web::scope("/admin/fs")
            .service(web::resource("/doc").route(web::get().to(biz_status_doc)))
            .service(web::resource("/home").route(web::get().to(load_home_admin)))
            .service(web::resource("/create_dir").route(web::post().to(create_dir_admin)))
            .service(web::resource("/delete").route(web::post().to(delete_admin)))
            .service(web::resource("/copy").route(web::post().to(copy_admin)))
            .service(web::resource("/move").route(web::post().to(move_to_admin)))
            .service(web::resource("/rename").route(web::post().to(rename_admin))),
    );
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadHomeParams {
    user_id: UserId,
}

async fn load_home_admin(_id: Identity, params: Query<LoadHomeParams>) -> JsonResponse<DirTree> {
    let tree = service::load_home(params.user_id).await?;
    ApiResponse::Ok(tree)
}

async fn load_home(id: Identity) -> JsonResponse<DirTree> {
    let id = id.id()?.parse::<UserId>()?;
    let tree = service::load_home(id).await?;
    ApiResponse::Ok(tree)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateDirDto {
    pub parent_id: UserFileId,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminParams<T> {
    user_id: UserId,
    #[serde(flatten)]
    params: T,
}

async fn create_dir_admin(
    _id: Identity,
    params: Json<AdminParams<CreateDirDto>>,
) -> JsonResponse<CreateDirResp> {
    let AdminParams {
        user_id,
        params: CreateDirDto { parent_id, name },
    } = params.into_inner();
    let file_id = service::create_dir(user_id, parent_id, &name).await??;
    ApiResponse::Ok(CreateDirResp { file_id })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateDirResp {
    pub file_id: UserFileId,
}

async fn create_dir(id: Identity, params: Json<CreateDirDto>) -> JsonResponse<CreateDirResp> {
    let id = id.id()?.parse::<UserId>()?;
    let CreateDirDto { parent_id, name } = params.into_inner();
    let file_id = service::create_dir(id, parent_id, &name).await??;
    ApiResponse::Ok(CreateDirResp { file_id })
}

static UPLOAD_TASKS: &str = "upload-tasks";

async fn register_upload_task(
    params: Json<RegisterUploadTaskDto>,
    identity: Identity,
    req: HttpRequest,
) -> JsonResponse<RegisterUploadTaskResp> {
    let id = identity.id()?.parse::<UserId>()?;
    let resp = upload::register_upload_task(id, params.into_inner()).await??;
    let ss = req.get_session();
    let tasks: Option<HashSet<UploadTaskId>> = ss.get(UPLOAD_TASKS)?;
    let mut tasks = tasks.unwrap_or_default();
    tasks.insert(resp.task_id);
    ss.insert(UPLOAD_TASKS, tasks)?;
    ApiResponse::Ok(resp)
}

async fn get_upload_tasks(_id: Identity, req: HttpRequest) -> JsonResponse<Vec<UploadTaskDto>> {
    let ss = req.get_session();
    let tasks: Option<HashSet<UploadTaskId>> = ss.get(UPLOAD_TASKS)?;
    let Some(tasks) = tasks else {
        return ApiResponse::Ok(Default::default());
    };

    let resp = upload::get_upload_tasks(tasks).await?;
    ApiResponse::Ok(resp)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelUplodTask {
    task_id: UploadTaskId,
}

async fn del_upload_task(
    _id: Identity,
    params: Json<DelUplodTask>,
    req: HttpRequest,
) -> JsonResponse<()> {
    let DelUplodTask { task_id } = params.into_inner();

    upload::clear_upload_tasks(HashSet::from_iter(vec![task_id])).await?;
    del_session_upload_task(task_id, req)?;

    ApiResponse::Ok(())
}

fn del_session_upload_task(task_id: UploadTaskId, req: HttpRequest) -> anyhow::Result<()> {
    let session = req.get_session();
    let tasks: Option<HashSet<UploadTaskId>> = session.get(UPLOAD_TASKS)?;
    if let Some(mut tasks) = tasks {
        tasks.remove(&task_id);
        session.insert(UPLOAD_TASKS, tasks)?;
    };

    Ok(())
}

async fn clear_upload_tasks(_id: Identity, req: HttpRequest) -> JsonResponse<()> {
    let ss = req.get_session();
    let tasks: Option<HashSet<UploadTaskId>> = ss.get(UPLOAD_TASKS)?;
    let Some(tasks) = tasks else {
        return ApiResponse::Ok(Default::default());
    };
    ss.remove(UPLOAD_TASKS);
    upload::clear_upload_tasks(tasks).await?;
    ApiResponse::Ok(())
}

#[derive(MultipartForm)]
pub struct UploadSliceParams {
    chunk: Bytes,
    index: Text<u32>,
    #[multipart(rename = "taskId")]
    task_id: Text<String>,
}

pub async fn upload_slice(
    _id: Identity,
    MultipartForm(form): MultipartForm<UploadSliceParams>,
) -> JsonResponse<()> {
    upload::store_slice(form.task_id.parse()?, form.index.0, &form.chunk.data).await??;
    ApiResponse::Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadFinishedParam {
    task_id: UploadTaskId,
}

async fn upload_finished(
    _id: Identity,
    params: Json<UploadFinishedParam>,
    http_req: HttpRequest,
) -> JsonResponse<UploadedUserFile> {
    let UploadFinishedParam { task_id } = params.into_inner();
    let resp = upload::upload_finished(task_id).await??;

    del_session_upload_task(task_id, http_req)?;

    ApiResponse::Ok(resp)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteDto {
    file_ids: Vec<UserFileId>,
}

async fn delete(id: Identity, params: Json<DeleteDto>) -> JsonResponse<()> {
    let id = id.id()?.parse::<UserId>()?;
    let DeleteDto { file_ids } = params.into_inner();
    service::delete(id, file_ids).await??;
    ApiResponse::Ok(())
}

async fn delete_admin(_id: Identity, params: Json<AdminParams<DeleteDto>>) -> JsonResponse<()> {
    let AdminParams {
        user_id,
        params: DeleteDto { file_ids },
    } = params.into_inner();
    service::delete(user_id, file_ids).await??;
    ApiResponse::Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveToParams {
    from: Vec<UserFileId>,
    to: UserFileId,
}

async fn copy(id: Identity, params: Json<MoveToParams>) -> JsonResponse<()> {
    let id = id.id()?.parse::<UserId>()?;
    let MoveToParams { from, to } = params.into_inner();
    service::copy_to(id, from, to).await??;
    ApiResponse::Ok(())
}

async fn copy_admin(_id: Identity, params: Json<AdminParams<MoveToParams>>) -> JsonResponse<()> {
    let AdminParams {
        user_id,
        params: MoveToParams { from, to },
    } = params.into_inner();
    service::copy_to(user_id, from, to).await??;
    ApiResponse::Ok(())
}

async fn move_to(id: Identity, params: Json<MoveToParams>) -> JsonResponse<()> {
    let id = id.id()?.parse::<UserId>()?;
    let MoveToParams { from, to } = params.into_inner();
    service::move_to(id, from, to).await??;
    ApiResponse::Ok(())
}

async fn move_to_admin(_id: Identity, params: Json<AdminParams<MoveToParams>>) -> JsonResponse<()> {
    let AdminParams {
        user_id,
        params: MoveToParams { from, to },
    } = params.into_inner();
    service::move_to(user_id, from, to).await??;
    ApiResponse::Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameParams {
    file_id: UserFileId,
    new_name: String,
}

async fn rename(id: Identity, params: Json<RenameParams>) -> JsonResponse<()> {
    let id = id.id()?.parse::<UserId>()?;
    let RenameParams { file_id, new_name } = params.into_inner();
    service::rename(id, file_id, &new_name).await??;
    ApiResponse::Ok(())
}

async fn rename_admin(_id: Identity, params: Json<AdminParams<RenameParams>>) -> JsonResponse<()> {
    let AdminParams {
        user_id,
        params: RenameParams { file_id, new_name },
    } = params.into_inner();
    service::rename(user_id, file_id, &new_name).await??;
    ApiResponse::Ok(())
}
