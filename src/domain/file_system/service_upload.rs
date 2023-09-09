use std::collections::HashSet;

use super::file::{FileNode, UserFileId, VirtualPath};
use crate::{domain::user::user::UserId, ensure_ok, flake_id_func};

use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters, Debug)]
#[getset(get = "pub(crate)")]
pub struct UploadTask {
    id: i64,
    user_id: UserId,
    hash: String,
    parent_dir_id: UserFileId,
    state: UploadTaskState,
    uploaded_slices: HashSet<u32>,
    path: VirtualPath,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UploadTaskState {
    Completed(UserFileId),
    Pending,
}

impl UploadTask {
    flake_id_func!();

    pub fn new(user_id: UserId, hash: String, parent_dir: UserFileId, path: VirtualPath) -> Self {
        Self {
            id: Self::next_id(),
            user_id,
            hash,
            parent_dir_id: parent_dir,
            state: UploadTaskState::Pending,
            uploaded_slices: Default::default(),
            path,
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

pub enum FinishUploadTaskErr {
    NoSlice,
    HashNotMatch,
}

#[derive(Debug)]
pub enum CreateTaskErr {
    ParentNotDir,
    BadFileName,
}

pub fn create_task(
    target_dir: &FileNode,
    file_name: &str,
    hash: String,
) -> Result<UploadTask, CreateTaskErr> {
    use CreateTaskErr::*;

    ensure_ok!(target_dir.is_dir(), ParentNotDir);

    let path = target_dir
        .path()
        .join_child(file_name)
        .map_err(|_| BadFileName)?;

    let task = UploadTask::new(*target_dir.user_id(), hash, *target_dir.id(), path);

    Ok(task)
}
