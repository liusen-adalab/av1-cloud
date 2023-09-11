pub mod file_system;
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
        $crate::diesel_new_type!($type_name, ::diesel::sql_types::Bigint);

        impl $type_name {
            pub fn next_id() -> $type_name {
                use flaken::Flaken;
                use std::sync::{Mutex, OnceLock};
                static USER_ID_GENERATOR: OnceLock<Mutex<Flaken>> = OnceLock::new();
                let f = USER_ID_GENERATOR.get_or_init(|| Mutex::new(Flaken::default()));
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
                let id = String::deserialize(deserializer)?;
                let id = id.parse().map_err(serde::de::Error::custom)?;
                Ok(Self(id))
            }
        }
    };
}

#[cfg(test)]
mod test {
    #[test]
    fn t_id_warper() {
        id_wraper!(UserId);

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

        let json = serde_json::to_string(&user_s).unwrap();
        assert_eq!(json, format!(r#"{{"id":"{}"}}"#, next));

        let user_d: User = serde_json::from_str(&json).unwrap();
        assert_eq!(user_s, user_d);
    }
}
