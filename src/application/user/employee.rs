use serde::Deserialize;
use utils::db_pools::postgres::PgConn;

use crate::{
    biz_ok,
    domain::user::{
        employee::{Employee, EmployeeId, InviteCode},
        service::SanityCheck,
        Email, EmailFormatErr, Password, PasswordFormatErr,
    },
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::{email::EmailCodeSender, repo_employee},
    pg_tx,
};
use anyhow::Result;
use derive_more::*;

pub async fn generate_invite_code(invitor: EmployeeId) -> Result<String> {
    let invite_code = InviteCode::generate();
    repo_employee::save_invite_code(invitor, &invite_code).await?;
    Ok(invite_code.to_string())
}

#[derive(From)]
pub enum RegisterErr {
    EmailFormat(EmailFormatErr),
    PasswordFormat(PasswordFormatErr),
    SanityCheck(SanityCheck),
    NoInvitor,
    AlreadyRegistered,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployeeRegisterDto {
    email: String,
    email_code: String,
    password: String,
    invitation_code: String,
}

pub async fn register(user_dto: EmployeeRegisterDto) -> BizResult<EmployeeId, RegisterErr> {
    let email = ensure_biz!(Email::try_from(user_dto.email));
    ensure_biz!(
        EmailCodeSender::verify_email_code(&email, &user_dto.email_code).await?,
        SanityCheck::EmailCodeNotMatch
    );
    let password = ensure_biz!(Password::try_from_async(user_dto.password).await);

    pg_tx!(register_tx, email, password, user_dto.invitation_code)
}

pub async fn register_tx(
    email: Email,
    password: Password,
    invitation_code: String,
    conn: &mut PgConn,
) -> BizResult<EmployeeId, RegisterErr> {
    // find invitor
    let code = InviteCode::from(invitation_code);
    let invitor = ensure_exist!(
        repo_employee::get_invitor_id(&code).await?,
        RegisterErr::NoInvitor
    );
    // register
    let employee = Employee::create(email, password, invitor);

    // save
    ensure_biz!(
        repo_employee::save(&employee, conn)
            .await?
            .actually_effected(),
        RegisterErr::AlreadyRegistered
    );
    biz_ok!(*employee.id())
}

#[derive(From)]
pub enum LoginErr {
    PasswordFormat(PasswordFormatErr),
    EmailFormat(EmailFormatErr),
    SanityCheck(SanityCheck),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginDto {
    email: String,
    password: String,
}

pub async fn login(params: LoginDto) -> BizResult<EmployeeId, LoginErr> {
    let email = ensure_biz!(Email::try_from(params.email));
    pg_tx!(login_tx, email, params.password)
}

pub async fn login_tx(
    email: Email,
    password: String,
    conn: &mut PgConn,
) -> BizResult<EmployeeId, LoginErr> {
    let user = repo_employee::find(&email, conn).await?;
    let mut employee = ensure_exist!(user, SanityCheck::PasswordNotMatch);

    ensure_biz!(employee.login(&password).await);

    repo_employee::update(&employee, conn).await?;

    biz_ok!(*employee.id())
}

pub async fn logout(_id: EmployeeId) -> anyhow::Result<()> {
    Ok(())
}
