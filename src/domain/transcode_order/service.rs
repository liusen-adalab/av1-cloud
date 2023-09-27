use crate::domain::{
    file_system::file::FileNode,
    transcode_order::{OrderStatus, TaskStatus, TranscodeOrderId, TranscodeTask, TranscodeTaskId},
    user::user::UserId,
};

use super::{params::TranscodeTaskParams, TranscocdeOrder};

pub fn create_order(
    user_id: UserId,
    params: Vec<(FileNode, TranscodeTaskParams)>,
) -> TranscocdeOrder {
    let order_id = TranscodeOrderId::next_id();
    let tasks = params
        .into_iter()
        .map(|(file, params)| TranscodeTask {
            id: TranscodeTaskId::next_id(),
            virtual_path: file.path().to_str().to_string(),
            sys_file_id: file.file_data().unwrap().id,
            user_file_id: *file.id(),
            order_id,
            params,
            status: TaskStatus::Processing,
        })
        .collect();
    let order = TranscocdeOrder {
        id: order_id,
        user_id,
        status: OrderStatus::Processing,
        tasks,
    };
    order
}
