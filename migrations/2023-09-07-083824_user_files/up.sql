-- Your SQL goes here
CREATE TABLE user_files(
    id BIGINT NOT NULL,
    sys_file_id BIGINT,
    user_id BIGINT NOT NULL,
    parent_id BIGINT,

    at_dir VARCHAR NOT NULL,
    file_name VARCHAR NOT NULL,

    is_dir BOOLEAN NOT NULL,

    deleted BOOLEAN NOT NULL DEFAULT false,
    create_at TIMESTAMP NOT NULL DEFAULT  NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT  NOW(),
    
    UNIQUE(user_id, at_dir, file_name),
    PRIMARY KEY (id)
);

SELECT diesel_manage_updated_at('user_files');