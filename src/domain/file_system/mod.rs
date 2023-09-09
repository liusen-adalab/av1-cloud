pub mod file;
pub mod service;
pub mod service_upload;

#[macro_export]
macro_rules! flake_id_func {
    () => {
        pub(crate) fn next_id() -> i64 {
            use flaken::Flaken;
            use std::sync::{Mutex, OnceLock};
            static USER_ID_GENERATOR: OnceLock<Mutex<Flaken>> = OnceLock::new();
            let f = USER_ID_GENERATOR.get_or_init(|| Mutex::new(Flaken::default()));
            let mut lock = f.lock().unwrap();
            lock.next() as i64
        }
    };
    ($type:path) => {
        pub(crate) fn next_id() -> $type {
            use flaken::Flaken;
            use std::sync::{Mutex, OnceLock};
            static USER_ID_GENERATOR: OnceLock<Mutex<Flaken>> = OnceLock::new();
            let f = USER_ID_GENERATOR.get_or_init(|| Mutex::new(Flaken::default()));
            let mut lock = f.lock().unwrap();
            $type(lock.next() as i64)
        }
    };
}

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
            ::serde::Serialize,
            ::serde::Deserialize,
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
    };
}

#[cfg(test)]
mod test {
    #[test]
    fn t_id_warper() {
        id_wraper!(UserId);

        let a = UserId(1);
        let b = UserId(2);
        assert_ne!(a, b);

        assert_eq!(a.to_string(), "1");
        assert_eq!(b.to_string(), "2");

        assert_eq!(a, UserId::from(1));
        assert_eq!(b, UserId::from(2));

        use std::str::FromStr;
        assert_eq!(a, UserId::from_str("1").unwrap());
        assert_eq!(b, UserId::from_str("2").unwrap());

        let id1 = UserId::next_id();
        let id2 = UserId::next_id();

        assert!(id1 < id2);
    }
}
