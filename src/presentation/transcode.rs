use actix_identity::Identity;
use actix_web::web::{self, Json};
use serde::Deserialize;
use tracing::warn;
use utils::code;

use crate::{
    application::transcode::{
        self, CreateOrderErr, CreateOrderResp, TaskResult, TranscodeParamsDto,
    },
    domain::user::user::UserId,
    http::{ApiError, ApiResponse, ApiResult},
    status_doc,
};

code! {
    mod = "order";
    index = 13;
    err_trait = crate::http::HttpBizError;

    ---
    CreateOrder {
        file_not_fount = "文件不存在",
        file_is_dir = "该文件是一个文件夹",
        not_a_video = "文件文件不是一个视频"
    }
}

impl From<CreateOrderErr> for ApiError {
    fn from(value: CreateOrderErr) -> Self {
        match value {
            CreateOrderErr::FileNotFound => CREATE_ORDER.file_not_fount.into(),
            CreateOrderErr::CannotTransDir => CREATE_ORDER.file_is_dir.into(),
            CreateOrderErr::NotAVideo => CREATE_ORDER.not_a_video.into(),
        }
    }
}

status_doc!();

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/order")
            .service(web::resource("/transcode_result").route(web::post().to(transcode_done)))
            .service(web::resource("/create").route(web::post().to(create_order))),
    );
}

#[derive(Deserialize)]
pub struct CreateOrderParams {
    params: Vec<TranscodeParamsDto>,
}

pub async fn create_order(
    id: Identity,
    params: Json<CreateOrderParams>,
) -> ApiResult<CreateOrderResp> {
    let id = id.id()?.parse::<UserId>()?;
    let resp = transcode::create_order(id, params.into_inner().params).await??;
    ApiResponse::Ok(resp)
}

async fn transcode_done(params: Json<TaskResult<()>>) -> ApiResult<()> {
    if let Err(err) = transcode::task_done(params.into_inner()).await {
        warn!(?err, "transcode done failed");
    }
    ApiResponse::Ok(())
}
