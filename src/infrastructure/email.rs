use std::{fs::File, io::Read, path::PathBuf, sync::OnceLock};

use anyhow::Result;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::{redis_conn_switch::redis_conn, settings::get_settings};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct EmailCodeCfg {
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

pub struct EmailCodeSender<'a> {
    email: &'a str,
    code: u32,
}

impl<'a> EmailCodeSender<'a> {
    pub async fn try_build(email: &'a str, code: u32) -> Result<Option<EmailCodeSender<'a>>> {
        let key = format!("email:code_record:{}", &email);
        let conn = &mut redis_conn().await?;

        // 一分钟内只能发送一次验证码
        let set_ok: bool = redis::cmd("set")
            .arg(&[&key, "1", "EX", "60", "NX"])
            .query_async(conn)
            .await?;

        if set_ok {
            Ok(Some(Self { email, code }))
        } else {
            Ok(None)
        }
    }

    pub async fn send(&self) -> Result<()> {
        debug!(code = self.code, "sending email code");
        let config = &get_settings().email_code;
        let template = get_email_code_template();
        let body = template.replace("{{email_code}}", self.code.to_string().as_str());
        send_email(&config.from_full, &self.email, &config.subject, body).await?;
        Ok(())
    }

    pub async fn save(&self) -> Result<()> {
        debug!(code = self.code, "saving email code");
        let conn = &mut redis_conn().await?;

        // 5 分钟有效期，在验证码加一个计数器
        conn.set_ex(Self::key(&self.email), self.code * 10 + 5, 300)
            .await?;
        Ok(())
    }

    /// 校验邮箱验证码
    /// 如果正确，对应的验证码将被删除
    /// 如果错误，消耗验证的次数
    pub async fn verify_email_code(email: &str, code: &str) -> Result<bool> {
        let Some(sent_code) = Self::get_sent_code(email).await? else {
            return Ok(false);
        };

        let valid = sent_code.to_string() == code;
        if valid {
            let conn = &mut redis_conn().await?;
            let _: () = conn.del(Self::key(email)).await?;
        }

        Ok(valid)
    }

    /// 获取已发送给 `email` 的验证码
    ///
    /// # Notes
    ///
    /// 为了防止暴力破解，验证码被获取 5 次后会被删除
    pub async fn get_sent_code(email: &str) -> Result<Option<u32>> {
        let mut conn = redis_conn().await?;
        let key = &Self::key(email);
        let code: i64 = conn.decr(key, 1).await?;
        if code < 0 {
            return Ok(None);
        }
        let code = code as u32;

        let trial_num = code % 10;
        let code = match trial_num {
            0 => {
                let _: bool = conn.del(key).await?;
                Some(code / 10)
            }
            1..=4 => Some(code / 10),
            _ => {
                // 如果是其他，可能之前出现了异常情况
                warn!(email, code, "Something wired about email-code is going on.");
                let _: bool = conn.del(key).await?;
                None
            }
        };
        Ok(code)
    }

    fn key(email: &str) -> String {
        format!("email:code:{}", email)
    }
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

    use anyhow::bail;

    use crate::init_global;

    use super::*;

    #[tokio::test]
    async fn test_send_email() -> anyhow::Result<()> {
        init_global().await?;

        let to = "lzs@orientphoenix.com";
        let code = 123456;
        let Some(sender) = EmailCodeSender::try_build(to, code).await? else {
            bail!("cannot build sender");
        };
        sender.send().await?;
        sender.save().await?;
        let sent_code = EmailCodeSender::get_sent_code(to).await?;

        assert_eq!(sent_code.unwrap(), code);
        Ok(())
    }
}
