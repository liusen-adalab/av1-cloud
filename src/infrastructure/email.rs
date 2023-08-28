use std::{fs::File, io::Read, path::PathBuf, sync::OnceLock};

use anyhow::Result;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use rand::Rng;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{redis_conn_switch::redis_conn, settings::get_settings};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct EmailCode {
    pub from_full: String,
    pub from_addr: String,
    pub password: String,
    pub server: String,
    pub port: u16,
    pub subject: String,

    pub template_file: PathBuf,
}

static EAMIL_CODE_TEMPLATE: OnceLock<String> = OnceLock::new();

//  这个函数应该在服务初始化时被调用一次，以检测模板文件是否可以正常读取
pub fn load_email_code_template() -> Result<&'static String> {
    let path = &get_settings().email_code.template_file;
    // 约定文件名后尽量少更改，所以直接在代码中写死路径
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();

    Ok(EAMIL_CODE_TEMPLATE.get_or_init(|| content))
}

pub fn get_email_code_template() -> &'static String {
    EAMIL_CODE_TEMPLATE.get().unwrap()
}

/// 获取已发送给 `email` 的验证码
///
/// # notes
///
/// 为了防止暴力破解，验证码被获取 5 次后会被删除
pub async fn retrive_sent_code(email: &str) -> Result<Option<String>> {
    let mut conn = redis_conn().await?;
    let key = &redis_key_email_code(email);
    let code: Option<u32> = conn.decr(key, 1).await?;
    let Some(code) = code else {
        return Ok(None);
    };
    let trial_num = code % 10;
    let code = match trial_num {
        0 => {
            let _: bool = conn.del(key).await?;
            Some(code / 10)
        }
        1..=4 => Some(code / 10),
        _ => {
            let _: bool = conn.del(key).await?;
            None
        }
    };
    Ok(code.map(|c| c.to_string()))
}

pub fn redis_key_email_code(email: &str) -> String {
    format!("email_code:{}", email)
}

///  发送验证码，后续调用 [`query_email_code`] 获取已发送的验证码
///
/// # 参数
///
/// * `to` - 接收验证码的邮箱地址
pub async fn send_code(to: &str, fake: bool) -> Result<String> {
    let config = &get_settings().email_code;

    let template = get_email_code_template();
    let email_code: u32 = rand::thread_rng().gen_range(100_000..999_999);
    debug!(email_code, "email code sending");
    let body = template.replace("{{email_code}}", email_code.to_string().as_str());

    if !fake {
        send_email(&config.from_full, to, &config.subject, body).await?;
    }
    let mut conn = redis_conn().await?;
    let key = redis_key_email_code(to);
    // 5 分钟有效期，在验证码加一个计数器
    conn.set_ex(key, email_code * 10 + 5, 300).await?;

    Ok(email_code.to_string())
}

pub async fn send_email(
    from: &str,
    to: &str,
    subject: impl Into<String>,
    body: String,
) -> Result<()> {
    let config = &get_settings().email_code;
    let email = Message::builder()
        .from(from.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(body.to_string())?;

    let creds = Credentials::new(config.from_addr.to_string(), config.password.to_string());

    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.server)
            .unwrap()
            .credentials(creds)
            .port(config.port)
            .build();

    let response = mailer.send(email).await?;
    debug!(?response, "sent email successfully");
    Ok(())
}

#[cfg(test)]
mod test {

    use super::*;

    #[tokio::test]
    async fn test_send_email() -> anyhow::Result<()> {
        // init_global().await?;

        let to = "liu_zsen@163.com";
        let sent_code = send_code(&to, true).await?;
        dbg!(&sent_code);
        let code = retrive_sent_code(&to).await?;
        assert_eq!(sent_code, code.unwrap());

        Ok(())
    }
}
