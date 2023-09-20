use crate::{domain::file_system::file::SysFileId, infrastructure::repo_user_file};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct MediaInfo {
    pub general: GeneralInfo,
    pub video: VideoInfo,
    pub audio: Option<AudioInfo>,
    pub ext: MediaExtInfo,
}

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct MediaExtInfo {
    default_audio_track_id: Option<u32>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct GeneralInfo {
    #[serde(default)]
    Format: Option<String>,

    #[serde(default)]
    VideoCount: Option<u8>,

    #[serde(default)]
    AudioCount: Option<u8>,

    #[serde(default)]
    DataSize: Option<String>,

    #[serde(default)]
    Duration: Option<f64>,

    #[serde(default)]
    pub OverallBitRate: Option<u32>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct VideoInfo {
    #[serde(default)]
    pub Format: Option<String>,
    #[serde(default)]
    Format_Level: Option<String>,
    #[serde(default)]
    Format_Profile: Option<String>,
    #[serde(default)]
    Format_Settings_CABAC: Option<String>,
    #[serde(default)]
    Format_Settings_GOP: Option<String>,
    #[serde(default)]
    Format_Settings_RefFrames: Option<String>,

    #[serde(default)]
    CodecID: Option<String>,

    #[serde(default)]
    pub Duration: Option<f64>,

    pub durationMs: Option<u32>,

    #[serde(default)]
    pub BitRate: Option<u32>,

    #[serde(default)]
    pub Width: Option<u32>,

    #[serde(default)]
    pub Height: Option<u32>,

    #[serde(default)]
    DisplayAspectRatio: Option<String>,
    #[serde(default)]
    FrameRate_Mode: Option<String>,

    #[serde(default)]
    FrameRate: Option<f64>,

    #[serde(default)]
    pub FrameCount: Option<u32>,

    #[serde(default)]
    ColorSpace: Option<String>,
    #[serde(default)]
    ChromaSubsampling: Option<String>,

    #[serde(default)]
    BitDepth: Option<u8>,

    #[serde(default)]
    ScanType: Option<String>,

    #[serde(default)]
    StreamSize: Option<u64>,

    #[serde(default)]
    colour_range: Option<String>,
    #[serde(default)]
    colour_primaries: Option<String>,

    #[serde(default)]
    transfer_characteristics: Option<String>,
    #[serde(default)]
    matrix_coefficients: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct AudioInfo {
    #[serde(default)]
    Format: Option<String>,

    #[serde(default)]
    Duration: Option<f64>,

    #[serde(default)]
    BitRate_Mode: Option<String>,

    #[serde(default)]
    BitRate: Option<u32>,

    #[serde(default)]
    Channels: Option<u8>,

    ChannelLayout: Option<String>,

    #[serde(default)]
    FrameRate: Option<f64>,

    #[serde(default)]
    Compression_Mode: Option<String>,

    #[serde(default)]
    Default: Option<String>,

    #[serde(default)]
    StreamSize: Option<u64>,
}

pub async fn file_parsed(file_id: SysFileId, video_parsed: Option<String>) -> Result<()> {
    debug!(%file_id, "file parsed, updating");
    let video_parsed = video_parsed.map(|v| serde_json::from_str(&v)).transpose()?;
    repo_user_file::update_file_matedata(file_id, video_parsed).await?;
    Ok(())
}
