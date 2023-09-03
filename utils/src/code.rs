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
            $($pub_err_item:tt = $tip:literal),* $(,)?
        })*

        ---
        $($tts:tt)*
    ) => {
        pub use code_inner::*;
        impl $trait for Err {
            fn code(&self) -> u32 {
                self.code
            }
        }

        mod code_inner {
            #![allow(non_upper_case_globals)]
            #![allow(non_snake_case)]
            #[derive(Debug, Copy, Clone)]
            pub struct Err {
                pub code: u32,
                pub msg: &'static str,
                pub tip: &'static str,
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
                    $(pub $pub_err_item: Err,)*
                }

                $crate::err_items!({$mod_name, $pub_err}, $pub_code, $($pub_err_item, $tip,)*);
                paste::paste! {
                    pub static [< $pub_err:snake:upper >]: $pub_err = $pub_err::generate();
                }

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

            $crate::doc_pub!(
            $(pub $pub_err = $pub_code{
                $($pub_err_item),*
            })*
            );

            $crate::code!(@build_endpoint {$mod_name, $index * 100}, $($tts)*);

            pub fn err_list() -> Vec<Document> {
                let mut doc_pub = doc_pub();
                let endpoint_docs =  $crate::doc_encpoints!($($tts)*);

                doc_pub.extend(endpoint_docs);
                doc_pub
            }

            pub fn doc_csv() -> String {
                let mut table = String::from("code, endpoint, msg, tip\n");
                for d in err_list() {
                    let row = &format!("{},{},{},{}\n", d.err.code, d.endpoint, d.err.msg, d.err.tip);
                    table += row;
                }

                table
            }
        }
    };

    (@build_endpoint {$mod_name:expr, $cur_endpoint_index:expr }, $endpoint:ident {$($fields:tt)*} $($tts:tt)* ) => {
       $crate::build_endpoint!(@build_endpoint {$mod_name, []}, {$endpoint, $cur_endpoint_index * 100,} $($fields)*,);
       $crate::code!(@build_endpoint {$mod_name, ($cur_endpoint_index + 1)}, $($tts)*);
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
        $prv_ty:ty = $tip:literal,
        $($tail:tt)*
    ) => {
        paste::paste!{
            struct [< $endpoint $prv_ty:camel >];

            const [< $endpoint $prv_ty>]: Err = Err {
                code: $cur_err_index,
                msg: concat!("/", $mod_name, "/", stringify!($endpoint), "/", stringify!($prv_ty)),
                tip: $tip
            };

            impl [< $endpoint $prv_ty:camel >] {
                const fn generate() -> Err {
                    [< $endpoint $prv_ty>]
                }
            }
        }

        paste::paste!{
           $crate::build_endpoint!{
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
          $crate::build_endpoint!{
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

            pub static [< $endpoint:snake:upper >]: $endpoint = $endpoint {
                $($field_name: $named_ty::generate(),)*
            };
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! err_items {
    ({$mod_name:literal, $err_ty:ident}, $cur_code:expr, $(,)?) => {};

    ({$mod_name:literal, $err_ty:ident}, $cur_code:expr, $item_name:ident, $tip:literal, $($tail:tt)*) => {
        paste::paste! {
            const [< $err_ty $item_name >]: Err = Err {
                code: $cur_code,
                msg: concat!("/", $mod_name, "/", stringify!($err_ty), "/", stringify!($item_name)),
                tip: $tip,
            };
            $crate::err_items!({$mod_name, $err_ty}, $cur_code + 1, $($tail)*);
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
                let docs: Vec<Vec<Document>>= vec![
                    $([< doc_ $endpoint >]() ,)*
                ];
            }
            let docs: Vec<Document> = docs.into_iter().flatten().collect();
            docs
        }
    };
}

pub trait Code: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static {
    fn code(&self) -> u32 {
        1
    }
}

/// 声明一个 Code trait。
///
/// 因为孤儿规则，如果直接在当前 crate 中声明，使用这个 trait 的其他 crate 就无法为一些常用 Error 实现这个 trait,
///
/// # Examples
/// ```
/// define_code_trait!();
///
/// code! {
///     mod = "user";
///     index = 10;
///     err_trait = Code;

///     pub Password = 20 {
///         too_long = "密码太长了",
///         too_short = "密码太短了"
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_code_trait {
    () => {
        pub trait Code: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static {
            fn code(&self) -> u32 {
                1
            }
        }
    };
}

#[cfg(test)]
mod test_code {
    #![allow(dead_code)]

    define_code_trait!();

    code! {
        mod = "user";
        index = 10;
        err_trait = Code;

        pub Password = 20 {
            too_long = "密码太长了",
            too_short = "密码太短了"
        }

        ---

        Register {
            use Password,
            alredy_register = "该用户已被注册",
            email_code_mismatch = "邮箱验证码错误",
        }

        Login {
            use Password,
            unregistered = "请先注册",
        }
    }

    #[test]
    fn t_code() {
        dbg!(REGISTER.password);
        dbg!(LOGIN.unregistered);
        dbg!(LOGIN.password);
        println!("{:?}", err_list());
    }
}
