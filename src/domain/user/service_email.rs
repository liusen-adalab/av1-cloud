use rand::Rng;

use crate::{
    biz_ok, domain::user::Email, ensure_exist, http::BizResult,
    infrastructure::email::EmailCodeSender,
};

use super::EmailFormatErr;

#[derive(derive_more::From)]
pub enum SendEmailCodeErr {
    Email(EmailFormatErr),
    TooFrequent,
}

pub async fn send_email_code(email: Email, fake: bool) -> BizResult<(), SendEmailCodeErr> {
    let code: u32 = rand::thread_rng().gen_range(100_000..999_999);
    let sender = ensure_exist!(
        EmailCodeSender::try_build(&**email, code).await?,
        SendEmailCodeErr::TooFrequent
    );
    if !fake {
        sender.send().await?;
    }
    sender.save().await?;

    biz_ok!(())
}

#[derive(derive_more::From)]
pub enum CheckEmailCodeErr {
    Email(EmailFormatErr),
    NoEmailCode,
}

pub async fn check_email_code(email: Email, code: &str) -> BizResult<bool, CheckEmailCodeErr> {
    let sent_code = EmailCodeSender::get_sent_code(&email).await?;
    let sent_code = ensure_exist!(sent_code, CheckEmailCodeErr::NoEmailCode).to_string();
    biz_ok!(sent_code == code)
}
