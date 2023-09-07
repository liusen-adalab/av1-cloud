use std::collections::HashSet;

use actix_identity::Identity;
use actix_multipart::form::bytes::Bytes;
use actix_multipart::form::text::Text;
use actix_multipart::form::{MultipartForm, MultipartFormConfig};
use actix_session::SessionExt;
use actix_web::web::{self, Json};
use actix_web::HttpRequest;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use utils::code;

use crate::application::file_system::upload::{
    self, FinishUploadTaskErr, RegisterUploadTaskDto, RegisterUploadTaskErr,
    RegisterUploadTaskResp, StoreSliceErr, UploadTaskDto, UploadedUserFile,
};
use crate::application::file_system::user_file::{self, CreateDirErr, DirTree};
use crate::domain::user::user::UserId;
use crate::http::{ApiError, ApiResponse};
use crate::{http::JsonResponse, status_doc};

code! {
    mod = "file_system";
    index = 12;
    err_trait = crate::http::HttpBizError;

    pub PathFormat = 200 {
        not_allow = "不允许的路径格式",
        too_long = "路径过长",
    }

    ---

    CreateDir {
        not_allowed = "不允许创建的目录路径",
        already_exist = "目录已存在，不允许重复创建",
        no_parent = "父目录不存在",
        parent_not_dir = "父级文件不是目录",
    }

    RegisterUploadTask {
        no_parent = "父目录不存在",
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

macro_rules! path_format_err {
    ($err:ident) => {{
        match $err {
            crate::domain::file_system::file::VirtualPathErr::NotAllowed => {
                PATH_FORMAT.not_allow.into()
            }
            crate::domain::file_system::file::VirtualPathErr::TooLong => {
                PATH_FORMAT.too_long.into()
            }
        }
    }};
}

impl From<RegisterUploadTaskErr> for ApiError {
    fn from(value: RegisterUploadTaskErr) -> Self {
        match value {
            RegisterUploadTaskErr::PathNotAllow(p) => path_format_err!(p),
            RegisterUploadTaskErr::NoParent => REGISTER_UPLOAD_TASK.no_parent.into(),
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
            FinishUploadTaskErr::SysBusy(_) => FINISH_UPLOAD.sys_busy.into(),
            FinishUploadTaskErr::NoParent => FINISH_UPLOAD.no_parent.into(),
            FinishUploadTaskErr::NoSlice => FINISH_UPLOAD.no_slice.into(),
        }
    }
}

impl From<CreateDirErr> for ApiError {
    fn from(value: CreateDirErr) -> Self {
        match value {
            CreateDirErr::PathErr(p) => path_format_err!(p),
            CreateDirErr::Create(c) => match c {
                crate::domain::file_system::service::CreateDirErr::NotAllowedPath => {
                    CREATE_DIR.not_allowed.into()
                }
            },
            CreateDirErr::AlreadyExist => CREATE_DIR.already_exist.into(),
            CreateDirErr::NoParent => CREATE_DIR.no_parent.into(),
            CreateDirErr::NotAllowed => CREATE_DIR.not_allowed.into(),
        }
    }
}

status_doc!();

pub fn actix_config(cfg: &mut web::ServiceConfig) {
    let m_limit = MultipartFormConfig::default().memory_limit(1024 * 1024 * 100);
    cfg.service(
        web::scope("/api/fs")
            .service(web::resource("/doc").route(web::get().to(get_resp_status_doc)))
            .service(web::resource("/home").route(web::get().to(load_home)))
            .service(web::resource("/create_dir").route(web::post().to(create_dir)))
            .service(
                web::resource("/register_upload_task").route(web::post().to(register_upload_task)),
            )
            .service(web::resource("/upload_tasks").route(web::get().to(get_upload_task)))
            .service(
                web::resource("/upload_slice")
                    .app_data(m_limit)
                    .route(web::post().to(upload_slice)),
            )
            .service(web::resource("/finish_upload").route(web::post().to(upload_finished))),
    );
}

async fn load_home(id: Identity) -> JsonResponse<DirTree> {
    let id = id.id()?.parse::<UserId>()?;
    let tree = user_file::load_home(id).await?;
    ApiResponse::Ok(tree)
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateDirDto {
    #[serde_as(as = "DisplayFromStr")]
    pub parent_id: i64,
    pub name: String,
}

#[serde_as]
#[derive(Serialize)]
struct CreateDirResp {
    #[serde_as(as = "DisplayFromStr")]
    pub file_id: i64,
}

async fn create_dir(id: Identity, params: Json<CreateDirDto>) -> JsonResponse<CreateDirResp> {
    let id = id.id()?.parse::<UserId>()?;
    let CreateDirDto { parent_id, name } = params.into_inner();
    let file_id = user_file::create_dir(id, parent_id, &name).await??;
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
    let tasks: Option<HashSet<i64>> = ss.get(UPLOAD_TASKS)?;
    let mut tasks = tasks.unwrap_or_default();
    tasks.insert(resp.task_id);
    ss.insert(UPLOAD_TASKS, tasks)?;
    ApiResponse::Ok(resp)
}

async fn get_upload_task(_id: Identity, req: HttpRequest) -> JsonResponse<Vec<UploadTaskDto>> {
    let ss = req.get_session();
    let tasks: Option<HashSet<i64>> = ss.get(UPLOAD_TASKS)?;
    let Some(tasks) = tasks else {
        return ApiResponse::Ok(Default::default());
    };

    let resp = upload::get_upload_tasks(tasks).await?;
    ApiResponse::Ok(resp)
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

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadFinishedParam {
    #[serde_as(as = "DisplayFromStr")]
    task_id: i64,
}

async fn upload_finished(
    _id: Identity,
    params: Json<UploadFinishedParam>,
) -> JsonResponse<UploadedUserFile> {
    let resp = upload::upload_finished(params.into_inner().task_id).await??;
    ApiResponse::Ok(resp)
}
