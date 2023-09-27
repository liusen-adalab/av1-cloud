use std::collections::HashSet;

use actix_web::web;
use serde::Serialize;

use crate::http::{ApiResponse, ApiResult};

pub mod employee;
pub mod file_system;
pub mod transcode;
pub mod user;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusCode {
    code: u32,
    msg: &'static str,
    endpoint: &'static str,
    tip: &'static str,
}

#[macro_export]
macro_rules! status_doc {
    () => {
        use super::StatusCode;

        #[allow(unused)]
        pub async fn biz_status_doc() -> ApiResult<Vec<StatusCode>> {
            let doc = biz_status_doc_inner();
            ApiResponse::Ok(doc)
        }

        pub fn biz_status_doc_inner() -> Vec<StatusCode> {
            let doc = err_list()
                .into_iter()
                .map(|d| StatusCode {
                    code: d.err.code,
                    endpoint: d.endpoint,
                    msg: d.err.msg,
                    tip: d.err.tip,
                })
                .collect();
            doc
        }
    };
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/admin/doc").route(web::get().to(doc)))
        .service(web::resource("/api/doc").route(web::get().to(doc)));
}

pub async fn doc() -> ApiResult<Vec<StatusCode>> {
    let user_doc = user::biz_status_doc_inner();
    let fs_doc = file_system::biz_status_doc_inner();
    let employee_doc = employee::biz_status_doc_inner();
    let transcode_doc = transcode::biz_status_doc_inner();

    let mut doc = Vec::new();
    doc.extend(user_doc);
    doc.extend(fs_doc);
    doc.extend(employee_doc);
    doc.extend(transcode_doc);

    let mut uniques = HashSet::new();
    doc.retain(|d| uniques.insert(d.code));

    ApiResponse::Ok(doc)
}
