use actix_web::web;

use crate::http::{ApiResponse, JsonResponse};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/user").service(web::resource("/register").route(web::get().to(register))),
    );
}

pub async fn register() -> JsonResponse<()> {
    ApiResponse::Ok(())
}
