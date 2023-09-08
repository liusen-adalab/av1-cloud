use std::collections::HashSet;

use super::file::UserFileId;
use crate::{domain::user::user::UserId, flake_id_func};

use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters, Debug)]
#[getset(get = "pub(crate)")]
pub struct UploadTask {
    id: i64,
    user_id: UserId,
    hash: String,
    parent_dir_id: UserFileId,
    file_name: String,
    state: UploadTaskState,
    uploaded_slices: HashSet<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UploadTaskState {
    Completed(UserFileId),
    Pending,
}

impl UploadTask {
    flake_id_func!();

    pub fn new(user_id: UserId, hash: String, parent_dir: UserFileId, file_name: String) -> Self {
        Self {
            id: Self::next_id(),
            user_id,
            hash,
            parent_dir_id: parent_dir,
            file_name,
            state: UploadTaskState::Pending,
            uploaded_slices: Default::default(),
        }
    }

    pub fn finished(&mut self, file_id: UserFileId) {
        self.state = UploadTaskState::Completed(file_id);
    }

    pub(crate) fn slice_done(&mut self, index: u32) {
        self.uploaded_slices.insert(index);
    }

    pub(crate) fn is_completed(&self) -> bool {
        matches!(self.state, UploadTaskState::Completed(_))
    }
}
