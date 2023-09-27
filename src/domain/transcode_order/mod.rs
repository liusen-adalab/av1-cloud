use getset::Getters;

use self::params::TranscodeTaskParams;
use super::{
    file_system::file::{SysFileId, UserFileId},
    user::user::UserId,
};
use crate::id_wraper;

pub mod params;
pub mod service;

id_wraper!(TranscodeOrderId);
id_wraper!(TranscodeTaskId);

#[derive(Getters)]
#[getset(get = "pub")]
pub struct TranscocdeOrder {
    id: TranscodeOrderId,
    user_id: UserId,
    status: OrderStatus,
    #[getset(skip)]
    tasks: Vec<TranscodeTask>,
}

#[derive(Clone, Copy)]
#[repr(i16)]
pub enum OrderStatus {
    Processing,
    Ok,
    Failed,
    Cancelled,
}

#[derive(Getters)]
#[getset(get = "pub")]
pub struct TranscodeTask {
    id: TranscodeTaskId,
    virtual_path: String,
    sys_file_id: SysFileId,
    user_file_id: UserFileId,
    order_id: TranscodeOrderId,
    params: TranscodeTaskParams,
    status: TaskStatus,
}

#[derive(derive_more::IsVariant)]
#[repr(i16)]
pub enum TaskStatus {
    Processing,
    Ok,
    Failed(String),
    Cancelled,
}

impl TaskStatus {
    fn is_end(&self) -> bool {
        match self {
            TaskStatus::Processing => false,
            TaskStatus::Ok => true,
            TaskStatus::Failed(_) => true,
            TaskStatus::Cancelled => true,
        }
    }
}
impl TranscocdeOrder {
    pub fn tasks(&self) -> &[TranscodeTask] {
        &self.tasks
    }

    pub fn tasks_mut(&mut self) -> &mut [TranscodeTask] {
        &mut self.tasks
    }

    pub fn task_completed(&mut self, task_id: TranscodeTaskId, result: Result<(), String>) {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id() == &task_id) else {
            return;
        };
        if let Err(err) = result {
            task.status = TaskStatus::Failed(err);
        } else {
            task.status = TaskStatus::Ok;
        }

        if self.tasks.iter().all(|task| task.status.is_end()) {
            if self.tasks.iter().any(|t| t.status.is_ok()) {
                self.status = OrderStatus::Ok;
            } else {
                self.status = OrderStatus::Failed;
            }
        }
    }
}

mod convert {
    use std::borrow::Cow;

    use anyhow::bail;

    use crate::{
        domain::transcode_order::OrderStatus,
        infrastructure::repo_order::{OrderPo, OrderPoWraper, TranscodeTaskPo},
    };

    use super::{TaskStatus, TranscocdeOrder, TranscodeTask};

    impl TranscocdeOrder {
        pub fn to_po(&self) -> OrderPoWraper {
            let tasks = self.tasks.iter().map(|task| task.to_po(self)).collect();
            OrderPoWraper {
                order: OrderPo {
                    id: *self.id(),
                    user_id: *self.user_id(),
                    status: self.status as i16,
                },
                tasks,
            }
        }

        pub fn try_from_po(order: OrderPoWraper) -> anyhow::Result<Self> {
            let tasks = order
                .tasks
                .into_iter()
                .map(|task| TranscodeTask::try_from_po(task))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let order = Self {
                id: order.order.id,
                user_id: order.order.user_id,
                status: match order.order.status {
                    0 => OrderStatus::Processing,
                    1 => OrderStatus::Ok,
                    2 => OrderStatus::Failed,
                    3 => OrderStatus::Cancelled,
                    _ => bail!("invalid order status"),
                },
                tasks,
            };

            Ok(order)
        }
    }

    impl TranscodeTask {
        pub fn to_po(&self, order: &TranscocdeOrder) -> TranscodeTaskPo {
            TranscodeTaskPo {
                id: self.id,
                virtual_path: Cow::Borrowed(&self.virtual_path),
                sys_file_id: self.sys_file_id,
                user_file_id: self.user_file_id,
                order_id: self.order_id,
                user_id: order.user_id,
                params: serde_json::to_string(&self.params).unwrap(),
                status: match self.status {
                    TaskStatus::Processing => 0,
                    TaskStatus::Ok => 1,
                    TaskStatus::Failed(_) => 2,
                    TaskStatus::Cancelled => 3,
                },
                err_msg: match &self.status {
                    TaskStatus::Failed(err) => Some(Cow::Borrowed(err)),
                    _ => None,
                },
            }
        }

        pub fn try_from_po(po: TranscodeTaskPo) -> anyhow::Result<Self> {
            let params = serde_json::from_str(&po.params)?;
            let status = match po.status {
                0 => TaskStatus::Processing,
                1 => TaskStatus::Ok,
                2 => TaskStatus::Failed(po.err_msg.unwrap().into_owned()),
                3 => TaskStatus::Cancelled,
                _ => bail!("invalid task status"),
            };
            Ok(Self {
                id: po.id,
                virtual_path: po.virtual_path.into_owned(),
                sys_file_id: po.sys_file_id,
                user_file_id: po.user_file_id,
                order_id: po.order_id,
                params,
                status,
            })
        }
    }
}
