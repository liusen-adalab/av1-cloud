use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use self::{audio::AudioProcessParameters, zcode::ZcodeProcessParams};

#[derive(Serialize, Deserialize, Debug)]
pub struct TranscodeTaskParams {
    pub work_dir: PathBuf,
    pub path: PathBuf,
    pub dst_path: PathBuf,
    pub frame_count: u32,
    pub is_h264: bool,

    pub container: ContainerFormat,
    pub video: ZcodeProcessParams,
    pub audio: Option<AudioProcessParameters>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ContainerFormat {
    #[serde(rename = "mp4")]
    Mp4,
    #[serde(rename = "mkv")]
    Mkv,
    #[serde(rename = "webm")]
    Webm,
}

impl ContainerFormat {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::Webm => "webm",
        }
    }
}

pub mod zcode {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy)]
    #[serde(rename_all = "camelCase")]
    pub struct ZcodeProcessParams {
        pub is_hdr: bool,
        pub width: u32,
        pub height: u32,
        pub format: VideoFormat,
        pub resolution: Option<Resolution>,
        pub ray_tracing: Option<RayTracing>,
        pub quality: OutputQuality,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum VideoFormat {
        Av1,
        H264,
        H265,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
    pub enum Resolution {
        #[serde(rename = "_144p")]
        _144P,
        #[serde(rename = "_240p")]
        _240P,
        #[serde(rename = "_360p")]
        _360P,
        #[serde(rename = "_480p")]
        _480P,
        #[serde(rename = "_720p")]
        _720P,
        #[serde(rename = "_1080p")]
        _1080P,
        #[serde(rename = "_1440p")]
        _1440P,
        #[serde(rename = "_4k")]
        _4K,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
    #[repr(i16)]
    #[serde(rename_all = "camelCase")]
    pub enum RayTracing {
        Cg = 0,
        TvSeries = 8,
        Ordinary = 14,
        Lagacy = 25,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
    #[repr(i16)]
    #[serde(rename_all = "camelCase")]
    pub enum OutputQuality {
        Base = 5,
        High = 4,
        Top = 3,
    }

    impl RayTracing {
        pub fn to_str(self) -> &'static str {
            match self {
                RayTracing::Cg => "cg",
                RayTracing::TvSeries => "tvSeries",
                RayTracing::Ordinary => "ordinary",
                RayTracing::Lagacy => "lagacy",
            }
        }
    }

    impl OutputQuality {
        pub fn to_str(self) -> &'static str {
            match self {
                OutputQuality::Base => "base",
                OutputQuality::High => "high",
                OutputQuality::Top => "top",
            }
        }
    }

    impl Resolution {
        pub fn to_str(&self) -> &'static str {
            match self {
                Resolution::_144P => "144p",
                Resolution::_240P => "240p",
                Resolution::_360P => "360p",
                Resolution::_480P => "480p",
                Resolution::_720P => "720p",
                Resolution::_1080P => "1080p",
                Resolution::_1440P => "1440p",
                Resolution::_4K => "4k",
            }
        }
    }
}

pub mod audio {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct AudioProcessParameters {
        pub format: AudioFormat,
        pub resample: AudioResampleRate,
        pub bitrate: AudioBitRate,
        pub track: AudioTrack,
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum AudioFormat {
        AAC,
        OPUS,
    }

    impl AudioFormat {
        pub fn to_str(self) -> &'static str {
            match self {
                AudioFormat::AAC => "aac",
                AudioFormat::OPUS => "opus",
            }
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u32)]
    pub enum AudioResampleRate {
        _22050 = 22050,
        _44100 = 44100,
        _48000 = 48000,
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u32)]
    pub enum AudioBitRate {
        #[serde(rename = "_16")]
        _16 = 16,
        #[serde(rename = "_32")]
        _32 = 32,
        #[serde(rename = "_64")]
        _64 = 64,
        #[serde(rename = "_128")]
        _128 = 128,
        #[serde(rename = "_256")]
        _256 = 256,
        #[serde(rename = "_320")]
        _320 = 320,
        #[serde(rename = "_384")]
        _384 = 384,
        #[serde(rename = "_640")]
        _640 = 640,
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AudioTrack {
        #[serde(rename = "_1")]
        _1 = 1,
        #[serde(rename = "_2")]
        _2 = 2,
        #[serde(rename = "_5.1")]
        _51 = 6,
        #[serde(rename = "_7.1")]
        _71 = 8,
    }

    impl AudioBitRate {
        pub fn to_str(self) -> &'static str {
            match self {
                AudioBitRate::_16 => "16k",
                AudioBitRate::_32 => "32k",
                AudioBitRate::_64 => "64k",
                AudioBitRate::_128 => "128k",
                AudioBitRate::_256 => "256k",
                AudioBitRate::_320 => "320k",
                AudioBitRate::_384 => "384k",
                AudioBitRate::_640 => "640k",
            }
        }
    }

    impl AudioResampleRate {
        pub fn to_str(self) -> &'static str {
            match self {
                AudioResampleRate::_22050 => "22050",
                AudioResampleRate::_44100 => "44100",
                AudioResampleRate::_48000 => "48000",
            }
        }
    }

    impl AudioTrack {
        pub fn to_str(self) -> &'static str {
            match self {
                AudioTrack::_1 => "1",
                AudioTrack::_2 => "2",
                AudioTrack::_51 => "5.1",
                AudioTrack::_71 => "7.1",
            }
        }
    }
}
