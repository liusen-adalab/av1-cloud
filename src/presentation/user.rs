use actix_identity::Identity;
use actix_web::{
    web::{self, Json, Query},
    HttpMessage, HttpRequest,
};
use serde::{Deserialize, Serialize};

use crate::{
    application::user::{self, LoginDto, UserDto},
    code,
    domain::user::{
        service::{LoginErr, RegisterErr},
        service_email::{CheckEmailCodeErr, SendEmailCodeErr},
    },
    http::{ApiError, ApiResponse, JsonResponse},
};

code! {
    mod = "user";  // 模块名
    index = 10;    // 模块序号
    err_trait = crate::http::HttpBizError; // http 状态码 trait 的路径

    pub PasswordFormat = 20 {
        too_long,
        too_short,
        not_allowed_char,
        too_simple,
    }

    pub UserNameFormat = 30 {
        too_long,
        too_short,
        not_allowed_char
    }

    pub EmailFormat = 40 {
        invalid,
    }

    ---

    Register {
        use PasswordFormat,
        alredy_register,
        no_email_code,
        email_code_mismatch
    }

    Login {
        use PasswordFormat,
        account_mismatch,
    }

    SendEmailCode {
        use EmailFormat,
        too_frequent
    }

    CheckEmailCode {
        no_email_code
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

macro_rules! user_name_err {
    ($p:expr) => {
        match $p {
            crate::domain::user::UserNameFormatErr::TooLong => USER_NAME_FORMAT.too_long.into(),
            crate::domain::user::UserNameFormatErr::TooShort => USER_NAME_FORMAT.too_short.into(),
            crate::domain::user::UserNameFormatErr::NotAllowedChar => {
                USER_NAME_FORMAT.not_allowed_char.into()
            }
        }
    };
}

macro_rules! email_err {
    ($p:expr) => {
        match $p {
            crate::domain::user::EmailFormatErr::Invalid => EMAIL_FORMAT.invalid.into(),
        }
    };
}

impl From<RegisterErr> for ApiError {
    fn from(value: RegisterErr) -> Self {
        match value {
            RegisterErr::Name(n) => user_name_err!(n),
            RegisterErr::Password(p) => password_err!(p),
            RegisterErr::Email(e) => email_err!(e),
            RegisterErr::AlreadyRegister => REGISTER.alredy_register.into(),
            RegisterErr::NoEmailCode => REGISTER.no_email_code.into(),
            RegisterErr::EmailCodeMisMatch => REGISTER.email_code_mismatch.into(),
        }
    }
}

impl From<SendEmailCodeErr> for ApiError {
    fn from(value: SendEmailCodeErr) -> Self {
        match value {
            SendEmailCodeErr::Email(e) => email_err!(e),
            SendEmailCodeErr::TooFrequent => SEND_EMAIL_CODE.too_frequent.into(),
        }
    }
}

impl From<LoginErr> for ApiError {
    fn from(value: LoginErr) -> Self {
        match value {
            LoginErr::Password(a) => password_err!(a),
            LoginErr::Email(e) => email_err!(e),
            LoginErr::EmailOrPasswordWrong => LOGIN.account_mismatch.into(),
        }
    }
}

impl From<CheckEmailCodeErr> for ApiError {
    fn from(value: CheckEmailCodeErr) -> Self {
        match value {
            CheckEmailCodeErr::Email(e) => email_err!(e),
            CheckEmailCodeErr::NoEmailCode => CHECK_EMAIL_CODE.no_email_code.into(),
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/user")
            .service(web::resource("/check_register").route(web::get().to(check_register)))
            .service(web::resource("/check_email_code").route(web::get().to(check_email_code)))
            .service(web::resource("/register").route(web::post().to(register)))
            .service(web::resource("/login").route(web::post().to(login)))
            .service(web::resource("/ping").route(web::get().to(user_ping)))
            .service(web::resource("/logout").route(web::post().to(logout)))
            .service(web::resource("/send_email_code").route(web::get().to(send_email_code))),
    );
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRgisterdParams {
    email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRgisterdResp {
    is_registered: bool,
}

pub(crate) async fn check_register(
    params: Query<CheckRgisterdParams>,
) -> JsonResponse<CheckRgisterdResp> {
    let CheckRgisterdParams { email } = params.into_inner();
    let registerd = user::is_email_registerd(email).await?;
    ApiResponse::Ok(CheckRgisterdResp {
        is_registered: registerd,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckEmailCodeParams {
    email: String,
    code: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckEmailCodeResp {
    valid: bool,
}

pub(crate) async fn check_email_code(
    params: Query<CheckEmailCodeParams>,
) -> JsonResponse<CheckEmailCodeResp> {
    let CheckEmailCodeParams { email, code } = params.into_inner();
    let valid = user::check_email_code(email, code).await??;

    ApiResponse::Ok(CheckEmailCodeResp { valid })
}

pub(crate) async fn register(params: Json<UserDto>, req: HttpRequest) -> JsonResponse<()> {
    let id = user::register(params.into_inner()).await??;
    Identity::login(&req.extensions(), id.to_string())?;
    ApiResponse::Ok(())
}

pub(crate) async fn login(params: Json<LoginDto>, req: HttpRequest) -> JsonResponse<()> {
    let id = user::login(params.into_inner()).await??;
    Identity::login(&req.extensions(), id.to_string())?;
    ApiResponse::Ok(())
}

pub(crate) async fn logout(id: Identity) -> JsonResponse<()> {
    let user_id = id.id()?.parse()?;
    user::logout(user_id).await?;
    id.logout();
    ApiResponse::Ok(())
}

pub(crate) async fn user_ping(_id: Identity) -> &'static str {
    "pong"
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendEmailCodeParams {
    email: String,
    #[serde(default)]
    fake: bool,
}

pub async fn send_email_code(params: Query<SendEmailCodeParams>) -> JsonResponse<()> {
    let SendEmailCodeParams { email, fake } = params.into_inner();

    user::send_email_code(email, fake).await??;
    ApiResponse::Ok(())
}
