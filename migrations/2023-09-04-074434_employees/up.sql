-- Your SQL goes here
CREATE TABLE employees(
    id BIGINT NOT NULL,

    name VARCHAR(20) NOT NULL,
    mobile_number VARCHAR(16) UNIQUE,
    email VARCHAR(100) NOT NULL UNIQUE,

    -- 使用 argon2 处理后，长度固定为 97 字节，但有可能变更
    password CHAR(97) NOT NULL,

    last_login TIMESTAMPTz NOT NULL DEFAULT NOW(),
    
    invited_by BIGINT NOT NULL,
    role SMALLINT NOT NULL DEFAULT 0,

    create_at TIMESTAMPTz NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTz NOT NULL DEFAULT NOW(),

    PRIMARY KEY (id)
);

SELECT diesel_manage_updated_at('employees');