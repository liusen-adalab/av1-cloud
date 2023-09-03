pub mod email;
pub mod repo_employee;
pub mod repo_user;
pub mod sms_code;

#[must_use]
pub struct EffectedRow(usize);

impl EffectedRow {
    pub fn actually_effected(&self) -> bool {
        self.0 > 0
    }
}
