-- Your SQL goes here
CREATE TABLE transcode_tasks(
    id BIGSERIAL NOT NULL,

    virtual_path VARCHAR NOT NULL,
    sys_file_id BIGINT NOT NULL,
    user_file_id BIGINT NOT NULL,
    order_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    params Text NOT NULL,
    
    status smallint NOT NULL,
    err_msg Text,

    create_at TIMESTAMPTz NOT NULL DEFAULT  NOW(),
    updated_at TIMESTAMPTz NOT NULL DEFAULT  NOW(),
    PRIMARY KEY (id)
);

SELECT diesel_manage_updated_at('transcode_tasks');