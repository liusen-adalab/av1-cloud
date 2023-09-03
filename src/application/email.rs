use crate::{
    biz_ok,
    domain::user::{Email, EmailFormatErr},
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::email::EmailCodeSender,
};

#[derive(derive_more::From)]
pub enum SendEmailCodeErr {
    Email(EmailFormatErr),
    TooFrequent,
}

pub async fn send_email_code(email: String, fake: bool) -> BizResult<(), SendEmailCodeErr> {
    let email = ensure_biz!(Email::try_from(email));
    let sender = ensure_exist!(
        EmailCodeSender::try_build(&**email, fake).await?,
        SendEmailCodeErr::TooFrequent
    );
    sender.send().await?;

    biz_ok!(())
}

#[derive(derive_more::From)]
pub enum CheckEmailCodeErr {
    Email(EmailFormatErr),
    NoEmailCode,
}

pub async fn verify_email_code(email: String, code: String) -> BizResult<bool, CheckEmailCodeErr> {
    let email = ensure_biz!(Email::try_from(email));
    let sent_code = EmailCodeSender::get_sent_code(&email).await?;
    let sent_code = ensure_exist!(sent_code, CheckEmailCodeErr::NoEmailCode).to_string();
    biz_ok!(sent_code == code)
}
