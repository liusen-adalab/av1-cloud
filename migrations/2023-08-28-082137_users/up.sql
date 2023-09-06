CREATE TABLE users(
    id BIGINT NOT NULL,

    name VARCHAR(20) NOT NULL,
    mobile_number VARCHAR(16) UNIQUE,
    email VARCHAR(100) NOT NULL UNIQUE,
    address VARCHAR(100),

    -- 使用 argon2 处理后，长度固定为 97 字节，但有可能变更
    password CHAR(97) NOT NULL,

    last_login TIMESTAMPTz NOT NULL DEFAULT NOW(),
    create_at TIMESTAMPTz NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTz NOT NULL DEFAULT NOW(),
    
    online BOOLEAN NOT NULL DEFAULT FALSE,

    PRIMARY KEY (id)
);

SELECT diesel_manage_updated_at('users');

COMMENT ON TABLE users IS '用户表;';
COMMENT ON COLUMN users.id IS '用户自增 id';
COMMENT ON COLUMN users.name IS '用户名';
COMMENT ON COLUMN users.mobile_number IS '手机号';
COMMENT ON COLUMN users.email IS '邮箱';
COMMENT ON COLUMN users.password IS '密码';
COMMENT ON COLUMN users.address IS '居住地';