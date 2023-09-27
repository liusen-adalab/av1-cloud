pub mod file_system;
pub mod transcode_order;
pub mod user;

#[macro_export]
macro_rules! id_wraper {
    ($type_name:ident) => {
        #[derive(
            ::derive_more::From,
            ::derive_more::Display,
            ::derive_more::FromStr,
            Debug,
            ::diesel:: AsExpression,
            ::diesel::FromSqlRow,
            PartialEq,
            PartialOrd,
            Eq,
            Hash,
            Clone,
            Copy,
        )]
        #[diesel(sql_type = ::diesel::sql_types::BigInt)]
        pub struct $type_name(pub i64);
        async_graphql::scalar!($type_name);
        $crate::diesel_new_type!($type_name, ::diesel::sql_types::Bigint);

        impl $type_name {
            pub fn next_id() -> $type_name {
                use flaken::Flaken;
                use std::sync::{Mutex, OnceLock};
                static USER_ID_GENERATOR: OnceLock<Mutex<Flaken>> = OnceLock::new();
                let f = USER_ID_GENERATOR.get_or_init(|| {
                    let ip = utils::process::get_local_ip_u32();
                    let f = flaken::Flaken::default();
                    let f = f.node(ip as u64);
                    Mutex::new(f)
                });
                let mut lock = f.lock().unwrap();
                $type_name(lock.next() as i64)
            }
        }
        impl ::redis::ToRedisArgs for $type_name {
            fn write_redis_args<W: ?Sized>(&self, out: &mut W)
            where
                W: redis::RedisWrite,
            {
                self.0.write_redis_args(out)
            }
        }

        impl ::redis::FromRedisValue for $type_name {
            fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
                let id = i64::from_redis_value(v)?;
                Ok($type_name(id))
            }
        }

        impl serde::Serialize for $type_name {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                serializer.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> serde::Deserialize<'de> for $type_name {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                #[derive(serde::Deserialize)]
                #[serde(untagged)]
                enum StringOrInt {
                    String(String),
                    Int(i64),
                }
                let id = StringOrInt::deserialize(deserializer)?;
                match id {
                    StringOrInt::String(id) => {
                        let id = id.parse().map_err(serde::de::Error::custom)?;
                        Ok(Self(id))
                    }
                    StringOrInt::Int(id) => Ok(Self(id)),
                }
            }
        }
    };
}

#[cfg(test)]
mod test {
    use utils::process::get_local_ip_u32;

    #[test]
    fn t_id_warper() {
        id_wraper!(UserId);

        let ip = get_local_ip_u32();
        let ip_node = (ip & ((1 << 10) - 1)) as u64;
        let id = UserId::next_id();
        let id_node = flaken::Flaken::default().decode(id.0 as u64).1;
        assert_eq!(ip_node, id_node);

        let a = UserId(1);
        assert_eq!(a.to_string(), "1");
        assert_eq!(a, UserId::from(1));

        use std::str::FromStr;
        assert_eq!(a, UserId::from_str("1").unwrap());

        let id1 = UserId::next_id();
        let id2 = UserId::next_id();
        assert!(id1 < id2);

        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
        struct User {
            id: UserId,
        }

        let next = UserId::next_id();
        let user_s = User { id: next };

        let json_str = serde_json::to_string(&user_s).unwrap();
        assert_eq!(json_str, format!(r#"{{"id":"{}"}}"#, next));

        let user_d: User = serde_json::from_str(&json_str).unwrap();
        assert_eq!(user_s, user_d);

        let json_int = format!(r#"{{"id":{}}}"#, next.0);
        let user_d: User = serde_json::from_str(&json_int).unwrap();
        assert_eq!(user_s, user_d);
    }
}
