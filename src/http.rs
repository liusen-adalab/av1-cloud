use std::fmt::{Debug, Display};

use actix_web::{body::BoxBody, http::StatusCode, web::Json, HttpResponse, ResponseError};
use serde::Serialize;

type Result<T, E = ApiError> = std::result::Result<T, E>;
pub type JsonResponse<T> = Result<Json<ApiResponse<T>>>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T: Serialize> {
    pub status: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err_msg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    #[allow(non_snake_case)]
    pub fn Ok(data: T) -> Result<Json<Self>> {
        Ok(Json(Self {
            status: 0,
            err_msg: None,
            data: Some(data),
        }))
    }
}

#[derive(derive_more::Display, Debug)]
pub struct ApiError {
    msg: Box<dyn HttpBizError>,
}

impl ApiError {
    pub fn code(&self) -> u32 {
        self.msg.code()
    }
}

pub trait HttpBizError: Display + Debug + Send + Sync + 'static {
    fn code(&self) -> u32 {
        1
    }
}

impl<T> From<T> for ApiError
where
    T: HttpBizError,
{
    fn from(value: T) -> Self {
        Self {
            msg: Box::new(value),
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let resp = ApiResponse::<()> {
            status: self.code(),
            err_msg: Some(self.to_string()),
            data: None,
        };
        HttpResponse::build(self.status_code()).json(resp)
    }
}

/// 一个用于生成业务状态码的声明宏
///
/// 这个宏会生成一系列错误结构体，同时会生成两个函数用于打印生成的错误列表：
/// 1. err_list()
/// 2. doc_csv()
///
/// # Examples
/// ```
///
/// code! {
///     mod = "user";  // 模块名
///     index = 10;    // 模块序号
///     err_trait = super::HttpBizError; // http 状态码 trait 的路径
///
///     pub Password = 20 {     // 通用的错误及序号
///         too_long,           // 20
///         too_short           // 21
///     }
///
///     ---                     // 分割线，下面是接口列表
///
///     Register {              // 注册接口
///         use Password,       // 使用通用的 Password 错误
///         alredy_register,    // 注册接口内部使用的错误
///         alredy_register2,   // 可以定义任意错误，但注意单个接口专属的错误最多只能有 9 个
///     }
///
///     Login {                 // 登录接口
///         use Password,       // 使用通用的 Password 错误
///         not_found,          // 定义登录接口专属的错误
///     }
/// }
///
/// fn test_code() {
///     dbg!(REGISTER.password.too_long);
///     dbg!(LOGIN.not_found);
///     println!("{}", doc_csv());
/// }
/// ```
#[macro_export]
macro_rules! code {
    (
        mod = $mod_name:literal;
        index = $index:literal;
        err_trait = $trait:path;

        $(pub $pub_err:ident = $pub_code:literal {
            $($pub_err_item:tt),* $(,)?
        })*

        ---
        $($tts:tt)*
    ) => {
        pub use code::*;
        impl $trait for Err {
            fn code(&self) -> u32 {
                self.code
            }
        }

        mod code {
            #![allow(non_upper_case_globals)]
            #![allow(non_snake_case)]
            #[derive(Debug, Copy, Clone)]
            pub struct Err {
                pub code: u32,
                pub msg: &'static str,
            }

            impl std::fmt::Display for Err {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "error occur: {}", self.msg)
                }
            }




            #[derive(Debug, Copy, Clone)]
            pub struct Document {
                pub err: Err,
                pub endpoint: &'static str
            }


            $(
                #[allow(non_snake_case)]
                #[derive(Debug, Copy, Clone)]
                pub struct $pub_err {
                    $($pub_err_item: Err,)*
                }

                crate::err_items!({$mod_name, $pub_err}, $pub_code, $($pub_err_item,)*);

                impl $pub_err {
                    pub const fn generate() -> Self {

                        paste::paste! {
                            $pub_err {
                                $($pub_err_item: [< $pub_err $pub_err_item >],)*
                            }
                        }
                    }
                }
            )*

            crate::doc_pub!(
            $(pub $pub_err = $pub_code{
                $($pub_err_item),*
            })*
            );

            code!(@build_endpoint {$mod_name, $index * 100}, $($tts)*);

            pub fn err_list() -> Vec<Document> {
                let mut doc_pub = doc_pub();
                let endpoint_docs =  crate::doc_encpoints!($($tts)*);

                doc_pub.extend(endpoint_docs);
                doc_pub
            }

            pub fn document() -> Vec<(u32, &'static str, &'static str)> {
                let errs = err_list();
                errs.into_iter()
                    .map(|d| (d.err.code, d.endpoint, d.err.msg))
                    .collect()
            }

            pub fn doc_csv() -> String {
                let mut table = String::from("code, endpoint, msg\n");
                for d in err_list() {
                    table += &format!("{},{},{}\n", d.err.code, d.endpoint, d.err.msg);
                }

                table
            }
        }
    };

    (@build_endpoint {$mod_name:expr, $cur_endpoint_index:expr }, $endpoint:ident {$($fields:tt)*} $($tts:tt)* ) => {
        build_endpoint!(@build_endpoint {$mod_name, []}, {$endpoint, $cur_endpoint_index * 100,} $($fields)*,);
        code!(@build_endpoint {$mod_name, ($cur_endpoint_index + 1)}, $($tts)*);
    };

    (@build_endpoint {$mod_name:expr, $cur_endpoint_index:expr }, $(,)? ) => {
    };
}

/// 执行一个返回值为 Result 的表达式，如果结果为 Err，打印一条错误日志
/// 用于只记录而不处理错误的情况
#[macro_export]
macro_rules! log_if_err {
    ($run:expr) => {
        crate::log_if_err!($run, stringify!($run))
    };

    ($run:expr, $msg:expr $(,)?) => {
        if let Err(err) = $run {
            ::tracing::error!(?err, concat!("FAILED: ", $msg))
        }
    };
}

/// 执行任意数量的表达式，任何一条的执行出错都会打印更丰富的信息，并直接抛出错误
#[macro_export]
macro_rules! log_err_ctx {
    ({$($runs:expr)*}) => {{
        let context = || {};
        crate::log_err_ctx!(@invoke $($runs)*, context)
    }};

    ({$($runs:expr)*} $(,$fields:ident)+ $(,)?) => {{
        let context = || {
            ::tracing::info!($(?$fields,)+);
        };
        crate::log_err_ctx!(@invoke $($runs)*, context)
    }};

    (@invoke $($runs:expr)*, $context:ident) => {{
        #[allow(redundant_semicolons)]
        {
            $(;crate::log_err_ctx!(@closure $runs, $context))*
        }
    }};

    (@closure $run:expr, $context:ident) => {{
        match $run {
            Ok(ok) => ok,
            Err(err) => {
                ::tracing::info!(concat!("FAILED: ", stringify!($run)));
                $context();
                return Err(err.into());
            }
        }
    }};

    ($run:expr $(, $fileds:ident)* $(,)?) => {{
        match $run {
            Ok(ok) => ok,
            Err(err) => {
                ::tracing::info!($(?$fileds,)* concat!("FAILED: ", stringify!($run)));
                return Err(err.into());
            }
        }
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! build_endpoint {
    (@build_endpoint
        {
            $mod_name:expr,
            [$($err_items:ident,)*]
        },
        {
            $endpoint:ident,
            $cur_err_index:expr,
            $(
                {
                    $field_name:ident,
                    $named_ty:ty,
                    $actual_ty:ty,
                }
            )*
        }
        $prv_ty:ty,
        $($tail:tt)*
    ) => {
        paste::paste!{
            struct [< $endpoint $prv_ty:camel >];

            const [< $endpoint $prv_ty>]: Err = Err {
                code: $cur_err_index,
                msg: concat!("/", $mod_name, "/", stringify!($endpoint), "/", stringify!($prv_ty))
            };

            impl [< $endpoint $prv_ty:camel >] {
                const fn generate() -> Err {
                    [< $endpoint $prv_ty>]
                }
            }
        }

        paste::paste!{
            build_endpoint!{
                @build_endpoint
                {
                    $mod_name,
                    [$($err_items,)* [< $endpoint $prv_ty>],]
                },
                {
                    $endpoint,
                    $cur_err_index + 10,
                    $(
                        {
                            $field_name,
                            $named_ty,
                            $actual_ty,
                        }
                    )*
                        {
                            [< $prv_ty:lower >],
                            [< $endpoint $prv_ty:camel >],
                            Err,
                        }
                }
                $($tail)*
            }
        }
    };

    (@build_endpoint
        {
            $mod_name:expr,
            [$($err_items:ident,)*]
        },
        {
            $endpoint:ident,
            $cur_err_index:expr,
            $(
                {
                    $field_name:ident,
                    $named_ty:ty,
                    $actual_ty:ty,
                }
            )*
        }
    use $pub_ty:ty,
    $($tail:tt)*
    ) => {
        paste::paste!{
            build_endpoint!{
                @build_endpoint
                {
                    $mod_name,
                    [$($err_items,)*]
                },
                {
                    $endpoint,
                    $cur_err_index,
                    $(
                        {
                            $field_name,
                            $named_ty,
                            $actual_ty,
                        }
                    )*
                        {
                            [< $pub_ty:lower >],
                            $pub_ty,
                            $pub_ty,
                        }
                }
                $($tail)*
            }
        }
    };


    (@build_endpoint
        {
            $mod_name:expr,
            [$($err_items:ident,)*]
        },
        {
            $endpoint:ident,
            $cur_err_index:expr,
            $(
                {
                    $field_name:ident,
                    $named_ty:ty,
                    $actual_ty:ty,
                }
            )*
        }
        $(,)?
    ) => {
        pub struct $endpoint {
            $(pub $field_name: $actual_ty,)*
        }

        paste::paste!{
            fn [< doc_ $endpoint >]() -> Vec<Document> {
                vec![
                    $(
                        Document {
                            err: $err_items,
                            endpoint: stringify!($endpoint)
                        },
                    )*
                ]
            }

            pub static [< $endpoint:upper >]: $endpoint = $endpoint {
                $($field_name: $named_ty::generate(),)*
            };
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! err_items {
    ({$mod_name:literal, $err_ty:ident}, $cur_code:expr, $(,)?) => {};

    ({$mod_name:literal, $err_ty:ident}, $cur_code:expr, $item_name:ident, $($tail:tt)*) => {
        paste::paste! {
            const [< $err_ty $item_name >]: Err = Err {
                code: $cur_code,
                msg: concat!("/", $mod_name, "/", stringify!($err_ty), "/", stringify!($item_name)),
            };
            crate::err_items!({$mod_name, $err_ty}, $cur_code + 1, $($tail)*);
        }
    };

}

#[doc(hidden)]
#[macro_export]
macro_rules! doc_pub {
    (
        $(
        pub $pub_err:ident = $pub_code:literal {
            $($pub_err_item:tt),* $(,)?
        }
        )*
    ) => {
        pub fn doc_pub() -> Vec<Document> {
            paste::paste! {
                vec![
                    $(
                        $(
                        Document {
                            err: [< $pub_err $pub_err_item >],
                            endpoint: "public"
                        },
                        )*
                    )*
                ]
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! doc_encpoints {
    ($($endpoint:ident {$($fields:tt)*})*) => {
        {
            paste::paste!{
                let docs = vec![
                    $([< doc_ $endpoint >]() ,)*
                ];
            }
            let docs: Vec<_> = docs.into_iter().flatten().collect();
            docs
        }
    };
}

#[cfg(test)]
mod test_code {
    #![allow(dead_code)]

    code! {
        mod = "user";
        index = 10;
        err_trait = super::HttpBizError;

        pub Password = 20 {
            too_long,
            too_short
        }

        ---

        Register {
            use Password,
            alredy_register,
            alredy_register2,
        }

        Login {
            use Password,
            alredy_register,
        }
    }

    #[test]
    fn t_code() {
        dbg!(REGISTER.password);
        dbg!(LOGIN.alredy_register);
        dbg!(LOGIN.password);
        println!("{}", doc_csv());
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! request {
    (method: $method:ident, client: $client:tt, url: $url:expr, $($tts:tt)* ) => {{
        use reqwest::header::CONTENT_TYPE;
        let req = $client.$method($url);
        let req = req.header(CONTENT_TYPE, "application/json");

        crate::request!(@config req, $($tts)*)
    }};

    //////////////////// config req ///
    (@config $req:expr $(,)?) => {{
        crate::request!(@do_request, $req)
    }};

    (@config $req:expr, header: {$($h_name:tt: $h_value:expr),* $(,)?} $($tts:tt)*) => {{
        let req = $req;
        $(let req = req.header($h_name, $h_value);)+
        crate::request!(@config req $($tts)*)
    }};

    (@config $req:expr, query: {$($key:literal: $value:expr),* $(,)?} $($tts:tt)*) => {{
        let req = $req;
        let q = ::serde_json::json!({
            $($key: $value),*
        });
        let req = req.query(&q);
        crate::request!(@config req $($tts)*)
    }};

    (@config $req:expr, query: $body:expr $(,)?) => {{
        let req = $req.query($body);
        crate::request!(@config req)
    }};

    (@config $req:expr, query: $body:expr, $($tts:tt)+) => {{
        let req = $req.query($body);
        crate::request!(@config req, $($tts)+)
    }};

    (@config $req:expr, body: {$($key:literal: $value:expr),* $(,)?} $($tts:tt)*) => {{
        let req = $req;
        let q = ::serde_json::json!({
            $($key: $value),*
        }).to_string();
        let req = req.body(q);
        crate::request!(@config req $($tts)*)
    }};

    (@config $req:expr, body: $body:expr $(,)?) => {{
        let req = $req.body($body);
        crate::request!(@do_post, req)
    }};

    (@config $req:expr, body: $body:expr, $($tts:tt)+) => {{
        let req = $req.body($body);
        crate::request!(@config req, $($tts)+)
    }};

    (@config $req:expr, timeout: $timeout:literal ms $($tts:tt)*) => {{
        let req = $req.timeout(::std::time::Duration::from_millis($timeout));
        crate::request!(@config req $($tts)*)
    }};

    (@config $req:expr, timeout: $timeout:literal s $($tts:tt)*) => {{
        let req = $req.timeout(::std::time::Duration::from_secs_f64($timeout));
        crate::request!(@config req $($tts)*)
    }};


    (@config $req:expr, ret: $resp_type:ident $(,)?) => {{
        crate::request!(@do_request, $req, $resp_type)
    }};

    ///////////// response ////
    (@do_request, $req:expr $(,$body_type:ident)?) => {{
        use ::anyhow::Context;
        let resp = match $req.send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::error!(?err, line = line!(), "[HTTP] failed to send request at: {}:{}", file!(), line!());
                return Err(err).context("[HTTP] send request failed").map_err(Into::into);
            }
        };

        if !resp.status().is_success() {
            tracing::error!(code=%resp.status(), ?resp, "[HTTP] http status error at: {}:{}", file!(), line!());
            return Err(anyhow::anyhow!("http request: code = {}", resp.status()).into())
        }
        $crate::extract_response_body!(GET, resp, $($body_type)?)
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! match_requst {
    (method: $method:tt, client: $client:expr, url: $url:expr $(,)?) => {{
        crate::request!(method: $method, client: $client, url: $url,)
    }};

    (method: $method:tt, client: $client:expr, url: $url:expr, $($tts:tt)+) => {{
        crate::request!(method: $method, client: $client, url: $url, $($tts)+)
    }};

    (method: $method:tt, $client:expr, $url:expr $(,)?) => {{
        crate::match_requst!(method: $method, client: $client, url: $url)
    }};

    (method: $method:tt, $client:expr, $url:expr, $($tts:tt)+) => {{
        crate::match_requst!(method: $method, client: $client, url: $url, $($tts)+)
    }};

    (method: $method:tt, $url:expr $(,)?) => {{
        let client_ = ::reqwest::Client::new();
        crate::match_requst!(method: $method, client: client_, url: $url)
    }};

    (method: $method:tt, $url:expr, $($tts:tt)+) => {{
        let client_ = ::reqwest::Client::new();
        crate::match_requst!(method: $method, client: client_, url: $url, $($tts)+)
    }};
}

/// 发起一个 http 请求，
///
/// # Examples
/// 最简形式
/// ```
/// let resp: serde_json::Value = get!("https://httpbin.org/ip");
/// ```
/// 完整形式
/// ```
/// let client = reqwest::Client::new();
/// let url = "https://httpbin.org/get";
/// let resp: serde_json::Value = get! {
///     client: client,     // 设置发起请求的 client，字段名可省略
///     url: url,           // 设置 url，字段名可省略
///                         // 以下 ４ 个字段名不能省略，但可以按任意顺序出现
///     header: {           // 增加请求 header
///         "aa": "bb",
///         "cc":"dd",
///     },
///     timeout: 3 s,      // 超时: 3s / 3000 ms / 3000_000 us
///     body: {"key": "value"},    // 请求体，使用 serde_json::json! 语法，也可以直接传入一个表达式
///     ret: json          // 如何读取返回值：json / text / bytes
/// };
/// println!("{}", resp);
/// ```
#[macro_export]
macro_rules! get {
    ($($tts:tt)+) => {{
        crate::match_requst!(method: get, $($tts)+)
    }};
}

/// 与 [`get`] 使用方式相同
#[macro_export]
macro_rules! post {
    ($($tts:tt)+) => {{
       crate::match_requst!(method: post, $($tts)+)
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! extract_response_body {
    ($method:tt, $resp:expr $(,)?) => {
        $crate::extract_response_body!($method, $resp, json)
    };

    ($method:tt, $resp:expr, text $(,)?) => {{
        match $resp.text().await {
            Ok(r) => r,
            Err(err) => {
                use anyhow::Context;
                let url = err.url();
                let is_redirect = err.is_redirect();
                ::tracing::error!(?err);
                ::tracing::error!(
                    ?url,
                    is_redirect,
                    concat!(
                        "[",
                        stringify!($method),
                        "]",
                        " failed to read response body as TEXT at: {}:{}"
                    ),
                    file!(),
                    line!()
                );
                return Err(err).context("http body cannot be read as text").map_err(Into::into);
            }
        }
    }};

    ($method:tt, $resp:expr, json $(,)?) => {{
        let url = $resp.url().clone();
        let text = $crate::extract_response_body!($method, $resp, text);
        match ::serde_json::from_str(&text) {
            Ok(r) => r,
            Err(err) => {
                use anyhow::Context;
                ::tracing::error!(%text, "origin response");
                ::tracing::error!(%url, concat!(
                        "[",
                        stringify!($method),
                        "]",
                        "failed to deserialize response body as json. At: {}:{}"
                    ),
                    file!(),
                    line!()
                );
                return Err(err).context("http body cannot be read as json").map_err(Into::into);
            }
        }
    }};

    ($method:tt, $resp:expr, $body_type:tt $(,)?) => {
        match $resp.$body_type().await {
            Ok(r) => r,
            Err(err) => {
                ::tracing::error!(
                    ?err,
                    concat!(
                        "[",
                        stringify!($method),
                        "]",
                        " failed to read response body at: {}:{}"
                    ),
                    file!(),
                    line!()
                );
                return Err(err.into());
            }
        }
    };
}

#[cfg(test)]
mod test_reqest {
    use anyhow::Result;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn t_get() -> Result<()> {
        let url = "https://httpbin.org/ip";
        let resp: serde_json::Value = get!(url, timeout: 3.5 s, ret: json);
        println!("{}", resp);

        let url = "https://httpbin.org/headers";
        let resp: serde_json::Value = get!(url, header: {"aa": "bb", "cc":"dd",},  ret: json);
        println!("{}", resp);

        let client = reqwest::Client::new();
        let url = "https://httpbin.org/get";
        let resp: serde_json::Value = get! {
            client: client,
            url: url,
            query: {"key": "value"},
            header: {
                "aa": "bb",
                "cc":"dd",
            },
            ret: json
        };
        println!("{}", resp);

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn t_post() -> Result<()> {
        let client = reqwest::Client::new();
        let url = "https://httpbin.org/post";
        let resp: serde_json::Value = post! {
            client: client,
            url: url,
            header: {
                "aa": "bb",
                "cc":"dd",
            },
            timeout: 3 ms,
            body: r#"{"key": "value"}"#,
            ret: json
        };
        println!("{}", resp);

        Ok(())
    }
}
