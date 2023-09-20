-- Your SQL goes here
CREATE TABLE sys_files(
    id BIGSERIAL NOT NULL,
    hash VARCHAR NOT NULL UNIQUE,
    path VARCHAR NOT NULL,

    size BIGINT NOT NULL,
    is_video BOOLEAN,
    
    transcode_from BIGINT,
    
    can_be_encode BOOLEAN,
    slice_count INT,

    -- 视频常用信息，冗余
    bit_rate INTEGER,
    duration_ms INTEGER,
    height INT,
    width INT,

    -- 视频信息
    general_info TEXT,
    video_info TEXT,
    audio_info TEXT,

    create_at TIMESTAMPTz NOT NULL DEFAULT  NOW(),
    updated_at TIMESTAMPTz NOT NULL DEFAULT  NOW(),

    PRIMARY KEY (id)
);

SELECT diesel_manage_updated_at('sys_files');