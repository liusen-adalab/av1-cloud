use derive_more::Deref;

pub mod email;
pub mod file_sys;
pub mod repo_employee;
pub mod repo_upload_task;
pub mod repo_user;
pub mod repo_user_file;
pub mod sms_code;

#[must_use]
pub struct EffectedRow(usize);

impl EffectedRow {
    pub fn is_effected(&self) -> bool {
        self.0 > 0
    }
}

#[derive(Deref, Default, Debug, Clone)]
pub struct RedisKey(String);

impl RedisKey {
    pub fn new(prefix: impl ToString) -> Self {
        Self(prefix.to_string())
    }

    pub fn add_field(self, field: impl AsRef<str>) -> Self {
        Self(self.0 + ":" + field.as_ref())
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[macro_export]
macro_rules! pg_exist {
    ($table:expr, $conn:expr, $($filter:expr),+ $(,)?) => {{
            let exist = diesel::select(diesel::dsl::exists(
                $table
                $(.filter($filter))*
            ))
            .get_result($conn)
            .await?;
            Ok(exist)
    }};
}
