use actix_identity::Identity;
use actix_session::SessionExt;
use actix_web::web::{self, Json};
use actix_web::{HttpMessage, HttpRequest};
use utils::code;

use crate::application::user::employee::{
    self, EmployeeRegisterDto, LoginDto, LoginErr, RegisterErr,
};
use crate::http::{ApiError, ApiResponse};
use crate::log_if_err;
use crate::{http::JsonResponse, status_doc};

use super::user::{EMAIL_FORMAT, PASSWORD_FORMAT, SANITY_CHECK};

code! {
    mod = "employee";
    index = 11;
    err_trait = crate::http::HttpBizError;

    ---

    Register {
        alredy_register = "账号已被注册，请直接登录",
        no_email_code = "请先获取邮箱验证码，再进行注册",
        invitation_code_not_match = "邀请码不正确，请重新填写"
    }
}

macro_rules! password_err {
    ($p:expr) => {
        match $p {
            crate::domain::user::PasswordFormatErr::TooLong => PASSWORD_FORMAT.too_long.into(),
            crate::domain::user::PasswordFormatErr::TooShort => PASSWORD_FORMAT.too_short.into(),
            crate::domain::user::PasswordFormatErr::NotAllowedChar => {
                PASSWORD_FORMAT.not_allowed_char.into()
            }
            crate::domain::user::PasswordFormatErr::TooSimple => PASSWORD_FORMAT.too_simple.into(),
        }
    };
}

macro_rules! sanity_check {
    ($s:expr) => {{
        match $s {
            crate::domain::user::SanityCheck::EmailCodeNotMatch => {
                SANITY_CHECK.email_code_not_match.into()
            }
            crate::domain::user::SanityCheck::SmsCodeNotMatch => {
                SANITY_CHECK.sms_code_not_match.into()
            }
            crate::domain::user::SanityCheck::PasswordNotMatch => {
                SANITY_CHECK.password_not_match.into()
            }
        }
    }};
}

impl From<RegisterErr> for ApiError {
    fn from(value: RegisterErr) -> Self {
        match value {
            RegisterErr::EmailFormat(..) => EMAIL_FORMAT.invalid.into(),
            RegisterErr::PasswordFormat(err) => password_err!(err),
            RegisterErr::SanityCheck(err) => sanity_check!(err),
            RegisterErr::NoInvitor => REGISTER.invitation_code_not_match.into(),
            RegisterErr::AlreadyRegistered => REGISTER.alredy_register.into(),
        }
    }
}
impl From<LoginErr> for ApiError {
    fn from(value: LoginErr) -> Self {
        match value {
            LoginErr::EmailFormat(..) => EMAIL_FORMAT.invalid.into(),
            LoginErr::PasswordFormat(err) => password_err!(err),
            LoginErr::SanityCheck(err) => sanity_check!(err),
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/admin/employee")
            .service(web::resource("/doc").route(web::get().to(biz_status_doc)))
            .service(web::resource("/invite_code").route(web::get().to(generate_invite_code)))
            .service(web::resource("/register").route(web::post().to(register)))
            .service(web::resource("/login").route(web::post().to(login)))
            .service(web::resource("/logout").route(web::post().to(logout))),
    );
}

status_doc!();

pub async fn generate_invite_code(id: Identity) -> JsonResponse<String> {
    let id = id.id()?.parse()?;
    let code = employee::generate_invite_code(id).await?;
    ApiResponse::Ok(code)
}

pub async fn register(params: Json<EmployeeRegisterDto>, req: HttpRequest) -> JsonResponse<()> {
    let params = params.into_inner();
    let (id, role) = employee::register(params.clone()).await??;

    Identity::login(&req.extensions(), id.to_string())?;
    let session = req.get_session();
    session.insert("role", role)?;

    ApiResponse::Ok(())
}

pub async fn login(params: Json<LoginDto>, req: HttpRequest) -> JsonResponse<()> {
    let (id, role) = employee::login(params.into_inner()).await??;

    Identity::login(&req.extensions(), id.to_string())?;
    let session = req.get_session();
    session.insert("role", role)?;

    // session
    ApiResponse::Ok(())
}

pub async fn logout(id: Identity) -> JsonResponse<()> {
    let user_id = id.id()?.parse()?;
    // 不返回错误，只记录日志
    log_if_err!(employee::logout(user_id).await);
    id.logout();
    ApiResponse::Ok(())
}
