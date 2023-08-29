use crate::{
    biz_ok, domain::user::Email, ensure_biz, ensure_exist, http::BizResult, infrastructure,
    redis_conn_switch::redis_conn,
};

use super::EmailFormatErr;

#[derive(derive_more::From)]
pub enum SendEmailCodeErr {
    Email(EmailFormatErr),
    TooFrequent,
}

pub async fn send_email_code(email: Email, fake: bool) -> BizResult<(), SendEmailCodeErr> {
    let key = format!("email_code_record:{}", &**email);
    let conn = &mut redis_conn().await?;
    let set_ok: bool = redis::cmd("set")
        .arg(&[&key, "1", "EX", "60", "NX"])
        .query_async(conn)
        .await?;
    ensure_biz!(set_ok, SendEmailCodeErr::TooFrequent);

    infrastructure::email::send_code(&email, fake).await?;

    biz_ok!(())
}

#[derive(derive_more::From)]
pub enum CheckEmailCodeErr {
    Email(EmailFormatErr),
    NoEmailCode,
}

pub async fn check_email_code(email: Email, code: &str) -> BizResult<bool, CheckEmailCodeErr> {
    let sent_code = infrastructure::email::retrive_sent_code(&**email).await?;
    let sent_code = ensure_exist!(sent_code, CheckEmailCodeErr::NoEmailCode);
    biz_ok!(sent_code == code)
}
