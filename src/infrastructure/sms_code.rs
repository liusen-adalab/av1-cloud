use anyhow::{bail, Result};
use chrono::{TimeZone, Utc};
use hmac::digest::CtOutput;
use hmac::Hmac;
use rand::{thread_rng, Rng};
use redis::AsyncCommands;
use reqwest::header::HeaderMap;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use tracing::{debug, error, warn};

pub fn get_user_key(tel: &str) -> String {
    format!("user:{}:smsCode", tel)
}

/*
推荐使用就近地域接入域名。根据调用接口时客户端所在位置，会自动解析到最近的某个具体地域的服务器。
例如在广州发起请求，会自动解析到广州的服务器
注意：对时延敏感的业务，建议指定带地域的域名。
 */
static HOST: &str = "sms.ap-guangzhou.tencentcloudapi.com"; // 华南地区(广州)

// 应用相关
static APP_ID: &str = "1400796999";
// API密钥管理 https://console.cloud.tencent.com/cam/capi
static SECRET_ID: &str = "AKIDVr0v2xgBRSCqHUTGp9E6mGWyGbibGhux";
static SECRET_KEY: &str = "tXAjFlIjz0rWIo8GBrKYZN0QefS0KSZl";
//
static SMS_TEMPLATE_ID: &str = "1707793";
static SIGN_CONTENT: &str = "东方凤鸣科技";
static CONTENT_TYPE: &str = "application/json";
static REGION: &str = "ap-guangzhou";
static SERVICE: &str = "sms";

type HmacSha256 = Hmac<Sha256>;

/// 短信API
pub struct SmsApi<'a> {
    method: &'static str,
    query: &'a str,
    action: &'static str,
    version: &'static str,
    template_id: &'a str,
    sign_name: &'a str,
    phone_number_set: Vec<&'a str>,
    template_param_set: Vec<&'a str>,
}

/// 最里层Response
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct InnerResponse1 {
    pub serial_no: String,
    pub phone_number: String,
    pub fee: u16,
    pub session_context: String,
    pub code: String,
    pub message: String,
    pub iso_code: String,
}

/// 里层Response
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct InnerResponse {
    pub send_status_set: Vec<InnerResponse1>,
    pub request_id: String,
}

/// 短信验证码的Response
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct SMSResponse {
    pub response: InnerResponse,
}

impl<'a> SmsApi<'a> {
    /// `new()` 创建一个发送手机验证码的结构体
    ///
    /// 最多不可超过**200**个手机号, 且要求全为**境内**手机号
    ///
    /// 不对手机号码做**校验**
    ///
    /// # Examples
    ///
    /// ```
    /// use service_user::sms_code::SmsApi;
    ///
    /// #[tokio::main]
    /// async fn main(){
    ///     let list = ["13333333333", "14444444444"].to_vec();
    ///     // 索引0为验证码, 索引1为 显式告知用户有效期为多长时间
    ///     let param: Vec<&str> = Vec::from(["123456", "5"]);
    ///
    ///     let sms = SmsApi::new(&list, &param).unwrap();
    ///     sms.send().await.unwrap();
    /// }
    /// ```
    ///
    ///
    ///
    pub fn new(tel_list: Vec<&'a str>, template_param: Vec<&'a str>) -> Result<Self> {
        if tel_list.len() > 200 || tel_list.is_empty() {
            bail!("手机号不允许超过200个 / 请检查手机号列表是否为空")
        }

        // 构造请求体
        Ok(SmsApi {
            action: "SendSms",
            version: "2021-01-11",
            //
            method: "POST",
            query: "",
            template_id: SMS_TEMPLATE_ID,
            sign_name: SIGN_CONTENT,
            // 私有参数
            phone_number_set: tel_list,
            template_param_set: template_param,
        })
    }

    /// 添加Headers
    fn add_headers(&self, timestamp: i64) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert("Content-Type", CONTENT_TYPE.parse().unwrap());
        headers.insert("Host", HOST.parse().unwrap());
        headers.insert("X-TC-Action", self.action.parse().unwrap());
        headers.insert("X-TC-Version", self.version.parse().unwrap());
        headers.insert("X-TC-Region", REGION.parse().unwrap());
        headers.insert("X-TC-Timestamp", timestamp.to_string().parse().unwrap());

        headers
    }

    /// `send()` 发送手机验证码的方法
    ///
    /// # Examples
    ///
    /// ```
    /// use service_user::sms_code::SmsApi;
    ///
    /// #[tokio::main]
    /// async fn main(){
    ///     let list = ["13333333333", "14444444444"].to_vec();
    ///     let param: Vec<&str> = Vec::from(["123456", "5"]);
    ///
    ///     let sms = SmsApi::new(&list, &param).unwrap();
    ///     sms.send().await.unwrap();
    /// }
    /// ```
    pub async fn send(&self) -> Result<SMSResponse, reqwest::Error> {
        // 定义当前时间戳
        let timestamp = Utc::now().timestamp();

        // 添加Header
        let mut headers = self.add_headers(timestamp);

        // 默认请求
        let body = if self.sign_name.is_empty() {
            json!({
                "PhoneNumberSet": self.phone_number_set,
                "SmsSdkAppId": APP_ID,
                "TemplateId": SMS_TEMPLATE_ID,
                "SignName": SIGN_CONTENT,
                "TemplateParamSet": self.template_param_set
            })
            .to_string()
        }
        // 自定义请求
        else if self.template_param_set.is_empty() {
            // 若无模板参数，则设置为空。
            json!({
                "PhoneNumberSet": self.phone_number_set,
                "SmsSdkAppId": APP_ID,
                "TemplateId": self.template_id,
                "SignName": self.sign_name
            })
            .to_string()
        } else {
            json!({
                "PhoneNumberSet": self.phone_number_set,
                "SmsSdkAppId": APP_ID,
                "TemplateId": self.template_id,
                "SignName": self.sign_name,
                "TemplateParamSet": self.template_param_set
            })
            .to_string()
        };

        // 向头部添加Authorization字段
        headers.insert(
            "Authorization",
            self.get_authorization(timestamp, body.as_str())
                .parse()
                .unwrap(),
        );

        match Client::post(&Client::new(), format!("https://{}", HOST))
            .headers(headers)
            .body(body)
            .send()
            .await
            .unwrap()
            .json::<SMSResponse>()
            .await
        {
            Ok(r) => Ok(r),
            Err(e) => {
                error!("{e}");
                Err(e)
            }
        }
    }

    /// 生成Authorization的整个过程
    fn get_authorization(&self, timestamp: i64, body: &str) -> String {
        // 拼接规范请求串
        let mut canonical_request = String::with_capacity(1000);

        // HTTPRequestMethod
        canonical_request.push_str(self.method);
        canonical_request.push('\n');

        // CanonicalURI
        canonical_request.push('/');
        canonical_request.push('\n');

        // CanonicalQueryString
        match self.method {
            "POST" => canonical_request.push_str(""),
            "GET" => canonical_request.push_str(self.query),
            _ => unreachable!(),
        }
        canonical_request.push('\n');

        // 获取日期 UTC+0
        let date = self.get_utc_date(timestamp);

        canonical_request.push_str(format!("content-type:application/json\nhost:sms.ap-guangzhou.tencentcloudapi.com\nx-tc-action:sendsms\nx-tc-region:ap-guangzhou\nx-tc-timestamp:{}\nx-tc-version:2021-01-11\n\n",timestamp).as_str());
        let signed_headers =
            "content-type;host;x-tc-action;x-tc-region;x-tc-timestamp;x-tc-version";
        canonical_request.push_str(signed_headers);
        canonical_request.push('\n');

        let hashed_request_payload =
            self.string_into_sha256_into_hex_into_lowercase(body.as_bytes());

        match self.method {
            "POST" => canonical_request.push_str(hashed_request_payload.as_str()),
            "GET" => canonical_request.push_str(""),
            _ => unreachable!(),
        }

        // 拼接待签名字符串
        let credential_scope = format!("{}/{}/tc3_request", date, SERVICE);

        let string_to_sign = format!(
            "TC3-HMAC-SHA256\n{}\n{}\n{}",
            timestamp,
            credential_scope,
            self.string_into_sha256_into_hex_into_lowercase(canonical_request.as_bytes())
        );

        // Signature 计算签名
        let secret_date =
            self.hmac_sha256(format!("TC3{}", SECRET_KEY).as_bytes(), date.as_bytes());
        let secret_service =
            self.hmac_sha256(secret_date.into_bytes().as_slice(), SERVICE.as_bytes());
        let secret_signing = self.hmac_sha256(
            secret_service.into_bytes().as_slice(),
            "tc3_request".as_bytes(),
        );

        let signature = hex::encode(
            self.hmac_sha256(
                secret_signing.into_bytes().as_slice(),
                string_to_sign.as_bytes(),
            )
            .into_bytes(),
        );

        let authorization = format!(
            "TC3-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            SECRET_ID, credential_scope, signed_headers, signature
        );

        authorization
    }

    // String -> sha256加密 -> 转换成16进制 -> 小写String
    fn string_into_sha256_into_hex_into_lowercase(&self, msg: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(msg);
        let result = hasher.finalize();

        // 16进制编码
        hex::encode(result).to_lowercase()
    }

    // HMAC sha256加密
    fn hmac_sha256(&self, key: &[u8], msg: &[u8]) -> CtOutput<Hmac<Sha256>> {
        use hmac::Mac;

        // 要获取底层数组，请使用 `into_bytes`，但要小心，因为代码值的不正确使用可能会导致定时攻击，从而破坏 `CtOutput` 提供的安全性
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
        mac.update(msg);
        // `result` 的类型为 `CtOutput`，它是字节数组的简单的封装，用于提供恒定的时间相等性检查
        mac.finalize()
    }

    /// Date 必须从时间戳 X-TC-Timestamp 计算得到，且时区为 UTC+0。
    /// 如果加入系统本地时区信息，例如东八区，将导致白天和晚上调用成功，但是凌晨时调用必定失败。
    /// 假设时间戳为 1551113065，在东八区的时间是 2019-02-26 00:44:25，但是计算得到的 Date 取 UTC+0 的日期应为 2019-02-25，而不是 2019-02-26。
    fn get_utc_date(&self, timestamp: i64) -> String {
        Utc.timestamp_opt(timestamp, 0)
            .unwrap()
            .date_naive()
            .to_string()
    }
}

use crate::redis_conn_switch::redis_conn;

pub async fn send_sms_code(tel_list: Vec<&str>, fake: bool) -> Result<()> {
    let mut conn = redis_conn().await?;

    // 生成验证码
    let n = thread_rng().gen_range(100000..999999).to_string();

    if fake {
        // 缓存验证码(有效期5分钟)
        conn.set_ex(get_user_key(tel_list[0]), n.as_str(), 300)
            .await?;
        debug!(code = n, "sent sms code");
        return Ok(());
    }

    // 发送验证码
    let sms_code_param: Vec<&str> = Vec::from([n.as_str(), "5"]);
    let api = SmsApi::new(tel_list.clone(), sms_code_param).unwrap();

    let response = api.send().await?;

    if &response.response.send_status_set[0].code == "Ok" {
        // 缓存验证码(有效期5分钟)
        conn.set_ex(get_user_key(tel_list[0]), n.as_str(), 300)
            .await?;
        Ok(())
    }
    // 捕获未知错误
    else {
        warn!("{:#?}", response);
        let code = response.response.send_status_set[0].code.clone();
        bail!(code)
    }
}

pub struct SmsSender<'a> {
    tel: &'a str,
    // 用于调试，如果为 true，则跳过发送验证码步骤
    fake: bool,
}

impl<'a> SmsSender<'a> {
    pub async fn try_build(tel: &'a str, fake: bool) -> Result<Option<SmsSender<'a>>> {
        let key = format!("sms:code_record:{}", &tel);
        let conn = &mut redis_conn().await?;

        // 一分钟内只能发送一次验证码
        let set_ok: bool = redis::cmd("set")
            .arg(&[&key, "1", "EX", "60", "NX"])
            .query_async(conn)
            .await?;

        if set_ok {
            Ok(Some(Self { tel, fake }))
        } else {
            Ok(None)
        }
    }

    pub async fn send(&self) -> Result<()> {
        let mut conn = redis_conn().await?;

        // 生成验证码
        let code: i64 = thread_rng().gen_range(100000..999999);

        let mut send_ok = self.fake;

        if !self.fake {
            // 发送验证码
            let code = &code.to_string();
            let sms_code_param: Vec<&str> = Vec::from([code, "5"]);
            let api = SmsApi::new(vec![&self.tel], sms_code_param).unwrap();
            let response = api.send().await?;

            send_ok = &response.response.send_status_set[0].code == "Ok";

            if !send_ok {
                warn!(?response, "Failed: send sms code");
                let code = response.response.send_status_set[0].code.clone();
                bail!(code)
            }
        }

        if send_ok {
            debug!(code, "sms code sent");
            // 5 分钟有效期，在验证码加一个计数器
            conn.set_ex(Self::key(&self.tel), code * 10 + 5, 300)
                .await?;
        }

        Ok(())
    }

    pub async fn verify(tel: &str, code: impl ToString) -> Result<bool> {
        let Some(sent) = Self::get_sent_code(tel).await? else {
            return Ok(false);
        };
        let valid = sent.to_string() == code.to_string();
        if valid {
            let conn = &mut redis_conn().await?;
            let _: () = conn.del(Self::key(tel)).await?;
        }

        Ok(valid)
    }

    /// 获取已发送给 `tel` 的验证码
    ///
    /// # Notes
    ///
    /// 为了防止暴力破解，验证码被获取 5 次后会被删除
    async fn get_sent_code(tel: &str) -> Result<Option<u32>> {
        let mut conn = redis_conn().await?;
        let key = &Self::key(tel);
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
                warn!(tel, code, "Something wired about email-code is going on.");
                let _: bool = conn.del(key).await?;
                None
            }
        };
        Ok(code)
    }

    fn key(tel: &str) -> String {
        format!("user:sms_code:{}", tel)
    }
}

/// 查询数据库中是否有短信验证码
pub async fn get_sent_sms_code(tel: &str) -> Result<Option<String>> {
    let mut conn = redis_conn().await?;

    let code = conn.get(get_user_key(tel)).await?;
    Ok(code)
}

#[cfg(test)]
mod test {
    use super::*;

    // 发送单个手机验证码
    #[tokio::test]
    async fn send_sms() {
        let tel = ["13129387413"].to_vec();
        let param = ["123456", "5"].to_vec();

        let sms = SmsApi::new(tel, param).unwrap();
        let response = sms.send().await;

        assert_eq!(response.unwrap().response.send_status_set[0].code, "Ok")
    }

    // 发送多个手机号验证码
    #[tokio::test]
    async fn send_sms_multi() {
        let tel = ["18645959590", "14707649560"].to_vec();
        let param = ["123456", "5"].to_vec();

        let sms = SmsApi::new(tel, param).unwrap();
        let response = sms.send().await;

        assert_eq!(response.unwrap().response.send_status_set[0].code, "Ok")
    }
}
