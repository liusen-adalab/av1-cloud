use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, path::Path};
use tracing::debug;

#[cfg(not(test))]
use crate::settings::get_settings;
use crate::{domain::file_system::file::SysFileId, id_wraper, post};

id_wraper!(TaskId);

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VideoTask<'a> {
    id: i64,
    file_id: i64,
    task: VideoTaskType<'a>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum VideoTaskType<'a> {
    Parse(Parse<'a>),
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Parse<'a> {
    path: Cow<'a, Path>,
}

#[allow(unused)]
#[derive(Deserialize, Debug)]
pub struct Av1FactoryCfg {
    endpoint: String,
}

#[allow(unused)]
#[derive(Deserialize)]
struct Av1FactoryResp<T> {
    status: u32,
    msg: Option<String>,
    data: Option<T>,
}

pub(crate) async fn parse_file(file_id: SysFileId, path: &Path) -> Result<()> {
    debug!(%file_id, "sending parse task request");
    let task = VideoTask {
        id: TaskId::next_id().0,
        file_id: file_id.0,
        task: VideoTaskType::Parse(Parse {
            path: Cow::Borrowed(path),
        }),
    };
    #[cfg(not(test))]
    let endpoint = &get_settings().av1_factory.endpoint;
    #[cfg(test)]
    let endpoint = "http://127.0.0.1:8993";

    let url = format!("{}/api/video/task", endpoint);
    let resp: Av1FactoryResp<()> = post!(url, body: serde_json::to_string(&task).unwrap());
    ensure!(resp.status == 0, "parse req error: {:?}", resp.msg);

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn aa() {
        let res = parse_file(1.into(), Path::new("/aa/bb")).await;
        let _ = dbg!(res);
    }
}
