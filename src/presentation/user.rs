use actix_identity::Identity;
use actix_web::{
    web::{self, Json, Query},
    HttpMessage, HttpRequest,
};
use serde::{Deserialize, Serialize};
use utils::code;

use crate::{
    application::user::{self, LoginDto, ResetPasswordDto, SendSmsCodeErr, UserDto, UserUpdateDto},
    domain::user::{
        service::{LoginErr, RegisterErr, ResetPasswordErr, UpdateProfileErr},
        service_email::{CheckEmailCodeErr, SendEmailCodeErr},
    },
    http::{ApiError, ApiResponse, JsonResponse},
    log_if_err,
};

code! {
    mod = "user";  // 模块名
    index = 10;    // 模块序号
    err_trait = crate::http::HttpBizError; // http 状态码 trait 的路径

    pub PasswordFormat = 20 {
        too_long = "密码太长了，请输入短于 16 个字符的密码",
        too_short = "密码太短了， 请输入长于 8 个字符的密码",
        not_allowed_char = "密码中包含不允许使用的字符，请输入字母、数字或下划线",
        too_simple = "密码太简单了，请输入包含字母、数字和下划线的密码",
    }

    pub UserNameFormat = 30 {
        too_long = "用户名太长了，请输入短于 20 个字符的用户名",
        too_short = "用户名太短了， 请输入长于 1 个字符的用户名",
        not_allowed_char = "用户名中包含不允许使用的字符，请重新输入",
    }

    pub EmailFormat = 40 {
        invalid = "请输入格式正确的邮箱",
    }

    pub PhoneFormatErr = 50 {
        invalid = "请输入格式正确的手机号",
    }

    pub SanityCheck = 60 {
        email_code_not_match = "邮箱验证码错误，请重新输入" ,
        sms_code_not_match= "手机验证码错误，请重新输入",
        password_not_match= "密码错误，请重新输入",
    }

    ---

    Register {
        use PasswordFormat,
        alredy_register= "账号已被注册，请直接登录",
        no_email_code= "请先获取邮箱验证码，再进行注册",
    }

    Login {
        use PasswordFormat,
        account_not_match = "账号或密码错误，请重新输入",
    }

    SendEmailCode {
        use EmailFormat,
        too_frequent = "获取邮箱验证码太频繁了，请稍后再试"
    }

    CheckEmailCode {
        no_email_code = "请先获取邮箱验证码"
    }

    ResetPassword{
        not_found = "账号不存在"
    }

    UpdateProfile {
        not_found = "账号不存在",
        phone_already_binded = "该手机号已被绑定"
    }

    SendSmsCode {
        too_frequent ="获取手机验证码太频繁了，请稍后再试"
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

macro_rules! phone_err {
    () => {
        PHONE_FORMAT_ERR.invalid.into()
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
            RegisterErr::Name(n) => user_name_err!(n),
            RegisterErr::Password(p) => password_err!(p),
            RegisterErr::Email(e) => email_err!(e),
            RegisterErr::AlreadyRegister => REGISTER.alredy_register.into(),
            RegisterErr::Sanity(s) => sanity_check!(s),
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
            LoginErr::EmailOrPasswordWrong => LOGIN.account_not_match.into(),
            LoginErr::Sanity(e) => sanity_check!(e),
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

impl From<ResetPasswordErr> for ApiError {
    fn from(value: ResetPasswordErr) -> Self {
        match value {
            ResetPasswordErr::Password(a) => password_err!(a),
            ResetPasswordErr::Email(e) => email_err!(e),
            ResetPasswordErr::SanityCheck(s) => sanity_check!(s),
            ResetPasswordErr::NotFound => RESET_PASSWORD.not_found.into(),
        }
    }
}

impl From<UpdateProfileErr> for ApiError {
    fn from(value: UpdateProfileErr) -> Self {
        match value {
            UpdateProfileErr::Name(a) => user_name_err!(a),
            UpdateProfileErr::Password(a) => password_err!(a),
            UpdateProfileErr::Phone(_) => PHONE_FORMAT_ERR.invalid.into(),
            UpdateProfileErr::NotFound => UPDATE_PROFILE.not_found.into(),
            UpdateProfileErr::Sanity(s) => sanity_check!(s),
            UpdateProfileErr::PhoneAlreadyBinded => UPDATE_PROFILE.phone_already_binded.into(),
        }
    }
}

impl From<SendSmsCodeErr> for ApiError {
    fn from(value: SendSmsCodeErr) -> Self {
        match value {
            SendSmsCodeErr::Phone(_) => phone_err!(),
            SendSmsCodeErr::TooFrequent => SEND_SMS_CODE.too_frequent.into(),
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/user")
            .service(web::resource("/doc").route(web::get().to(get_resp_status_doc)))
            .service(web::resource("/check_register").route(web::get().to(check_register)))
            .service(web::resource("/check_email_code").route(web::get().to(check_email_code)))
            .service(web::resource("/register").route(web::post().to(register)))
            .service(web::resource("/login").route(web::post().to(login)))
            .service(web::resource("/ping").route(web::get().to(user_ping)))
            .service(web::resource("/logout").route(web::post().to(logout)))
            .service(web::resource("/reset_password").route(web::post().to(reset_password)))
            .service(web::resource("/modify_info").route(web::post().to(update_profile)))
            .service(web::resource("/sms_code").route(web::get().to(send_sms_code)))
            .service(web::resource("/send_email_code").route(web::get().to(send_email_code))),
    )
    .service(
        web::scope("/admin/user")
            .service(web::resource("/modify").route(web::post().to(update_profile_by_employee))),
    );
}

#[derive(Serialize)]
pub struct StatusCode {
    code: u32,
    msg: &'static str,
    endpoint: &'static str,
    tip: &'static str,
}

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
    // 不返回错误，只记录日志
    log_if_err!(user::logout(user_id).await);
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

pub async fn reset_password(params: Json<ResetPasswordDto>) -> JsonResponse<()> {
    user::reset_password(params.into_inner()).await??;
    ApiResponse::Ok(())
}

pub async fn update_profile(id: Identity, params: Json<UserUpdateDto>) -> JsonResponse<()> {
    let user_id = id.id()?.parse()?;
    user::update_profile(user_id, params.into_inner()).await??;
    ApiResponse::Ok(())
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdateDtoByAdmin {
    user_id: String,
    #[serde(flatten)]
    new_profile: UserUpdateDto,
}

pub async fn update_profile_by_employee(
    _id: Identity,
    params: Json<UserUpdateDtoByAdmin>,
) -> JsonResponse<()> {
    let UserUpdateDtoByAdmin {
        user_id,
        new_profile,
    } = params.into_inner();
    let user_id = user_id.parse()?;
    user::update_profile(user_id, new_profile).await??;
    ApiResponse::Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendSmsCodeParams {
    mobile_number: String,
    #[serde(default)]
    fake: bool,
}

pub async fn send_sms_code(params: Query<SendSmsCodeParams>) -> JsonResponse<()> {
    let SendSmsCodeParams {
        mobile_number,
        fake,
    } = params.into_inner();

    user::send_sms_code(mobile_number, fake).await??;
    ApiResponse::Ok(())
}
