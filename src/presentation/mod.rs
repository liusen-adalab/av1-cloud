use serde::Serialize;

pub mod employee;
pub mod file_system;
pub mod user;

#[derive(Serialize)]
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

        pub async fn get_resp_status_doc() -> JsonResponse<Vec<StatusCode>> {
            let doc = err_list()
                .into_iter()
                .map(|d| StatusCode {
                    code: d.err.code,
                    endpoint: d.endpoint,
                    msg: d.err.msg,
                    tip: d.err.tip,
                })
                .collect();
            ApiResponse::Ok(doc)
        }
    };
}
