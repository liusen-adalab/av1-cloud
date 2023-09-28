use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use utils::db_pools::postgres::{pg_conn, PgConn};

use crate::domain::file_system::file::{UserFileId, VideoInfo, VirtualPath};
use crate::domain::file_system::service::path_manager;
use crate::domain::transcode_order::params::audio::AudioProcessParameters;
use crate::domain::transcode_order::params::zcode::{
    OutputQuality, RayTracing, Resolution, VideoFormat, ZcodeProcessParams,
};
use crate::domain::transcode_order::params::{ContainerFormat, TranscodeTaskParams};
use crate::domain::transcode_order::{service, TranscodeTaskId};
use crate::infrastructure::{av1_factory, repo_order, repo_user_file};
use crate::{biz_ok, ensure_biz, ensure_exist, tx_func};
use crate::{
    domain::{transcode_order::TranscodeOrderId, user::user::UserId},
    http::BizResult,
};
use anyhow::Result;

use super::file_system;

pub enum CreateOrderErr {
    FileNotFound,
    CannotTransDir,
    NotAVideo,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeParamsDto {
    pub file_id: UserFileId,
    pub include_audio: bool,
    pub container_format: ContainerFormat,
    pub video: ZcodeProcessParamsDto,
    #[serde(default)]
    pub audio: Option<AudioProcessParameters>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeProcessParamsDto {
    pub format: VideoFormat,
    pub resolution: Option<Resolution>,
    pub ray_tracing: Option<RayTracing>,
    pub quality: OutputQuality,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderResp {
    order_id: TranscodeOrderId,
    task_ids: Vec<TranscodeTaskId>,
}

pub async fn create_order(
    user_id: UserId,
    params: Vec<TranscodeParamsDto>,
) -> BizResult<CreateOrderResp, CreateOrderErr> {
    use CreateOrderErr::*;

    let mut transcode_params = vec![];
    for param in params {
        let file = ensure_exist!(
            repo_user_file::find_video(param.file_id).await?,
            FileNotFound
        );
        ensure_biz!(file.is_file(), CannotTransDir);
        let meta = file.file_data().unwrap();
        ensure_biz!(meta.video_info.is_some(), NotAVideo);
        let video = meta.video_info.as_ref().unwrap();

        let task_params = to_task_params(meta, video, param);
        transcode_params.push((file, task_params));
    }

    let order = service::create_order(user_id, transcode_params);
    for task in order.tasks() {
        av1_factory::transcode(*task.id(), *task.sys_file_id(), task.params())
            .await
            .context("send task request")?;
    }

    let conn = &mut pg_conn().await?;
    let _ = repo_order::save(&order, conn).await?;

    biz_ok!(CreateOrderResp {
        order_id: *order.id(),
        task_ids: order.tasks().iter().map(|t| *t.id()).collect(),
    })
}

fn to_task_params(
    meta: &crate::domain::file_system::file::FileNodeMetaData,
    video: &VideoInfo,
    param: TranscodeParamsDto,
) -> TranscodeTaskParams {
    let manager = path_manager();
    let work_dir = manager.transcode_work_dir(&meta.hash);

    let video_params = ZcodeProcessParams {
        is_hdr: video.hdr_format.is_some(),
        width: video.width,
        height: video.height,
        format: param.video.format,
        resolution: param.video.resolution,
        ray_tracing: param.video.ray_tracing,
        quality: param.video.quality,
    };
    let dst_path = manager.transcode_dst_path(
        &meta.hash,
        param.container_format,
        &video_params,
        &param.audio,
    );
    let task_params = TranscodeTaskParams {
        work_dir,
        path: meta.archived_path.clone(),
        dst_path,
        frame_count: video.frame_count,
        video: video_params,
        audio: param.audio,
        container: param.container_format,
        is_h264: video.is_h264,
    };
    task_params
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TaskResult<O> {
    pub task_id: TranscodeTaskId,
    pub file_id: i64,
    pub result: Result<O, String>,
}

pub async fn task_done(result: TaskResult<()>) -> Result<()> {
    tx_func!(task_done_tx, result)
}

pub async fn task_done_tx(result: TaskResult<()>, conn: &mut PgConn) -> Result<()> {
    debug!(?result, "transcode task done");

    let task_id = result.task_id;
    let Some(mut order) = repo_order::find(result.task_id, conn).await? else {
        warn!(%task_id, "order not found");
        return Ok(());
    };
    let user_id = *order.user_id();

    if let Err(err) = &result.result {
        info!(%err, "task failed");
        order.task_completed(task_id, result.result);
        let _ = repo_order::update(&order, conn).await?;
        return Ok(());
    }

    let task = order
        .tasks_mut()
        .iter_mut()
        .find(|task| task.id() == &task_id)
        .expect("task not found");

    let params = task.params();
    let hash = repo_user_file::get_hash(*task.user_file_id())
        .await?
        .ok_or_else(|| anyhow!("file not found"))?;

    let transcode_out_path =
        path_manager().transcode_dst_path(&hash, params.container, &params.video, &params.audio);
    let virtual_path = VirtualPath::build(user_id, task.virtual_path())
        .map_err(|_| anyhow!("invalid virtual path"))?;
    debug!("create transcoded file");
    let mut mirror_path = virtual_path.mirror_path();
    let out_name = transcode_out_path.file_name().unwrap().to_string_lossy();
    let new_name = format!("{}_{}", mirror_path.file_stem(), out_name);
    mirror_path.set_file_name(new_name).unwrap();
    file_system::service::create_user_file(transcode_out_path, mirror_path, conn)
        .await
        .context("create user file")?;

    order.task_completed(task_id, result.result);

    let _ = repo_order::update(&order, conn).await?;

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::domain::transcode_order::params::audio::{
        AudioBitRate, AudioFormat, AudioResampleRate, AudioTrack,
    };

    use super::*;
    #[test]
    fn json() {
        let a = TranscodeParamsDto {
            file_id: 12839.into(),
            container_format: ContainerFormat::Mkv,
            video: ZcodeProcessParamsDto {
                format: VideoFormat::Av1,
                resolution: Some(Resolution::_1080P),
                ray_tracing: Some(RayTracing::TvSeries),
                quality: OutputQuality::High,
            },
            audio: Some(AudioProcessParameters {
                format: AudioFormat::AAC,
                resample: AudioResampleRate::_22050,
                bitrate: AudioBitRate::_256,
                track: AudioTrack::_51,
            }),
            include_audio: true,
        };

        let b = serde_json::to_string_pretty(&a).unwrap();
        println!("{}", b);
    }
}
