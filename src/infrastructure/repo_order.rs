use std::borrow::Cow;

use crate::domain::file_system::file::{SysFileId, UserFileId};
use crate::domain::transcode_order::{TranscocdeOrder, TranscodeOrderId, TranscodeTaskId};
use crate::domain::user::user::UserId;
use crate::schema::{orders, transcode_tasks};

use super::EffectedRow;
use anyhow::Result;
use diesel::prelude::Queryable;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use utils::db_pools::postgres::PgConn;

pub struct OrderPoWraper<'a> {
    pub order: OrderPo,
    pub tasks: Vec<TranscodeTaskPo<'a>>,
}

#[derive(Queryable, Selectable, Insertable, AsChangeset, Identifiable, Debug)]
#[diesel(table_name = orders)]
pub struct OrderPo {
    pub id: TranscodeOrderId,
    pub user_id: UserId,
    pub status: i16,
}

#[derive(Queryable, Selectable, Insertable, AsChangeset, Identifiable, Debug)]
#[diesel(table_name = transcode_tasks)]
pub struct TranscodeTaskPo<'a> {
    pub id: TranscodeTaskId,
    pub virtual_path: Cow<'a, str>,
    pub sys_file_id: SysFileId,
    pub user_file_id: UserFileId,
    pub order_id: TranscodeOrderId,
    pub user_id: UserId,
    pub params: String,
    pub status: i16,
    pub err_msg: Option<Cow<'a, str>>,
}

pub enum OrderStatus {
    Processing,
    Ok,
    Failed,
    Cancelled,
}

pub async fn save(order: &TranscocdeOrder, conn: &mut PgConn) -> Result<EffectedRow> {
    let order_po = order.to_po();
    let expect = order_po.tasks.len() + 1;
    let o_e = diesel::insert_into(orders::table)
        .values(&order_po.order)
        .execute(conn)
        .await?;

    let t_e = diesel::insert_into(transcode_tasks::table)
        .values(&order_po.tasks)
        .execute(conn)
        .await?;

    Ok(EffectedRow {
        expect_row: expect,
        effected_row: o_e + t_e,
    })
}

diesel::joinable!(transcode_tasks -> orders (order_id));

pub async fn find(task_id: TranscodeTaskId, conn: &mut PgConn) -> Result<Option<TranscocdeOrder>> {
    let task: Option<(TranscodeTaskPo, OrderPo)> = transcode_tasks::table
        .find(task_id)
        .inner_join(orders::table)
        .select((TranscodeTaskPo::as_select(), OrderPo::as_select()))
        .first::<(TranscodeTaskPo, OrderPo)>(conn)
        .await
        .optional()?;

    let Some((task, order)) = task else {
        return Ok(None);
    };

    let mut others_tasks: Vec<TranscodeTaskPo> = transcode_tasks::table
        .filter(transcode_tasks::order_id.eq(task.order_id))
        .filter(transcode_tasks::id.ne(task.id))
        .select(TranscodeTaskPo::as_select())
        .load::<TranscodeTaskPo>(conn)
        .await?;

    others_tasks.push(task);

    let order = TranscocdeOrder::try_from_po(OrderPoWraper {
        order,
        tasks: others_tasks,
    })?;

    Ok(Some(order))
}

pub async fn update(order: &TranscocdeOrder, conn: &mut PgConn) -> Result<()> {
    let order = order.to_po();
    diesel::update(orders::table)
        .filter(orders::id.eq(order.order.id))
        .set(&order.order)
        .execute(conn)
        .await?;
    for task in order.tasks {
        diesel::update(transcode_tasks::table)
            .filter(transcode_tasks::id.eq(task.id))
            .set(&task)
            .execute(conn)
            .await?;
    }
    Ok(())
}
