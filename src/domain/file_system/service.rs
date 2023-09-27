use std::{path::PathBuf, sync::OnceLock};

use anyhow::ensure;

use crate::domain::{
    transcode_order::params::{
        audio::AudioProcessParameters, zcode::ZcodeProcessParams, ContainerFormat,
    },
    user::user::UserId,
};

use super::{file::VirtualPath, service_upload::UploadTaskId};

pub struct PathManager {
    #[allow(dead_code)]
    root: PathBuf,
    repo_root: PathBuf,
    uploading_dir: PathBuf,
    user_space: PathBuf,
}

static PATH_MANAGER: OnceLock<PathManager> = OnceLock::new();

pub fn path_manager() -> &'static PathManager {
    PATH_MANAGER.get().unwrap()
}

impl PathManager {
    pub fn init(root: PathBuf) -> anyhow::Result<&'static Self> {
        ensure!(root.is_absolute(), "storage root must be absolute");
        let manager = PathManager {
            repo_root: root.join("archived"),
            uploading_dir: root.join("uploading"),
            user_space: root.join("user-space"),
            root,
        };
        std::fs::create_dir_all(&manager.repo_root)?;
        std::fs::create_dir_all(&manager.uploading_dir)?;
        std::fs::create_dir_all(&manager.user_space)?;

        Ok(PATH_MANAGER.get_or_init(|| manager))
    }

    pub fn user_home(&self, user_id: UserId) -> PathBuf {
        self.user_space.join(user_id.to_string())
    }

    pub fn upload_slice_dir(&self, task_id: UploadTaskId) -> PathBuf {
        self.uploading_dir.join(task_id.to_string())
    }

    pub fn archived_dir(&self, hash: &str) -> PathBuf {
        self.repo_root.join(&hash)
    }

    pub fn archived_path(&self, hash: &str) -> PathBuf {
        self.archived_dir(hash).join("origin-file")
    }

    pub fn thumbnail_dir(&self, hash: &str) -> PathBuf {
        self.archived_dir(hash).join("thumbnails")
    }

    pub fn transcode_work_dir(&self, hash: &str) -> PathBuf {
        self.archived_dir(hash).join("transcode-work")
    }

    pub fn transcode_out_name(
        container: ContainerFormat,
        v_params: &ZcodeProcessParams,
        a_params: &Option<AudioProcessParameters>,
    ) -> String {
        let mut v_path = String::from("v_");
        v_path += match v_params.format {
            crate::domain::transcode_order::params::zcode::VideoFormat::Av1 => "av1",
            crate::domain::transcode_order::params::zcode::VideoFormat::H264 => "h264",
            crate::domain::transcode_order::params::zcode::VideoFormat::H265 => "h265",
        };
        if let Some(r) = v_params.resolution {
            v_path += "_";
            v_path += r.to_str();
        }

        v_path += "_";
        v_path += v_params.quality.to_str();

        if let Some(r) = v_params.ray_tracing {
            v_path += "_";
            v_path += r.to_str();
        }

        let a_path = a_params
            .as_ref()
            .map(|a_params| {
                let mut a_path = String::from("_a");

                a_path += "_";
                a_path += a_params.format.to_str();

                a_path += "_";
                a_path += a_params.bitrate.to_str();

                a_path += "_";
                a_path += a_params.resample.to_str();

                a_path += "_";
                a_path += a_params.track.to_str();

                a_path
            })
            .unwrap_or_default();

        format!("{}{}.{}", v_path, a_path, container.to_str())
    }

    pub fn transcode_dst_path(
        &self,
        hash: &str,
        container: ContainerFormat,
        v_params: &ZcodeProcessParams,
        a_params: &Option<AudioProcessParameters>,
    ) -> PathBuf {
        let out_name = Self::transcode_out_name(container, v_params, a_params);
        self.archived_dir(hash).join(out_name)
    }
}

impl PathManager {
    pub fn virtual_to_disk(virtual_path: &VirtualPath) -> PathBuf {
        let home = path_manager().user_home(virtual_path.user_id());
        virtual_path.to_disk_path(&home)
    }
}
