use serde::Deserialize;
use utils::db_pools::postgres::pg_conn;

use crate::{
    domain::user::{
        service::{self, login_tx, LoginErr, RegisterErr, ResetPasswordErr, UpdateProfileErr},
        service_email::{self, CheckEmailCodeErr, SendEmailCodeErr},
        Email, Password, Phone, User, UserId, UserName,
    },
    ensure_biz,
    http::BizResult,
    infrastructure::repo_user,
    pg_tx, tx_func,
};
use anyhow::Result;

pub async fn is_email_registerd(email: String) -> Result<bool> {
    let Ok(email) = Email::try_from(email) else {
        return Ok(false);
    };
    let conn = &mut pg_conn().await?;
    let exist = repo_user::exist(&email, conn).await?;
    Ok(exist)
}

pub async fn check_email_code(email: String, code: String) -> BizResult<bool, CheckEmailCodeErr> {
    let email = ensure_biz!(Email::try_from(email));
    service_email::check_email_code(email, &code).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    email: String,
    email_code: String,
    password: String,
}

pub async fn register(user_dto: UserDto) -> BizResult<UserId, RegisterErr> {
    let email = ensure_biz!(Email::try_from(user_dto.email));
    let password = ensure_biz!(Password::try_from_async(user_dto.password).await);
    let user = User::create(email, password);
    service::register(user, user_dto.email_code).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginDto {
    email: String,
    password: String,
}

pub async fn login(login: LoginDto) -> BizResult<UserId, LoginErr> {
    let email = ensure_biz!(Email::try_from(login.email));
    pg_tx!(login_tx, email, login.password)
}

pub async fn logout(id: UserId) -> anyhow::Result<()> {
    tx_func!(service::logout_tx, id)
}

pub async fn send_email_code(email: String, fake: bool) -> BizResult<(), SendEmailCodeErr> {
    let email = ensure_biz!(Email::try_from(email));
    service_email::send_email_code(email, fake).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordDto {
    email: String,
    new_password: String,
    email_code: String,
}

pub async fn reset_password(params: ResetPasswordDto) -> BizResult<(), ResetPasswordErr> {
    let email = ensure_biz!(Email::try_from(params.email));
    let new_password = ensure_biz!(Password::try_from_async(params.new_password).await);
    service::reset_password(email, new_password, params.email_code).await
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdateDto {
    pub user_name: Option<String>,
    pub password: Option<UpdatePassword>,
    pub address: Option<Vec<String>>,
    pub mobile_number: Option<MobileNumber>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePassword {
    old_password: String,
    new_password: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MobileNumber {
    // 管理员可以不传这个参数
    #[serde(default)]
    pub sms_code: String,
    pub tel: String,
}

pub async fn update_profile(
    user_id: UserId,
    update_info: UserUpdateDto,
) -> BizResult<(), UpdateProfileErr> {
    let phone = if let Some(phone) = update_info.mobile_number {
        // todo: verify sms code
        Some(ensure_biz!(Phone::try_from(phone.tel)))
    } else {
        None
    };
    let user_name = if let Some(name) = update_info.user_name {
        Some(ensure_biz!(UserName::try_from(name)))
    } else {
        None
    };

    let password = if let Some(password) = update_info.password {
        let p = service::UpdatePassword {
            old_password: password.old_password,
            new_password: ensure_biz!(Password::try_from_async(password.new_password).await),
        };
        Some(p)
    } else {
        None
    };

    let update_info = service::UserUpdate {
        user_name,
        password,
        address: update_info.address.map(|a| a.join(",")),
        mobile_number: phone,
    };

    pg_tx!(service::update_profile, user_id, update_info)
}
