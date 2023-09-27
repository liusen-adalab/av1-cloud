-- Your SQL goes here
CREATE TABLE orders(
    id BIGSERIAL NOT NULL,
    user_id BIGINT NOT NULL,
    status smallint NOT NULL,

    create_at TIMESTAMPTz NOT NULL DEFAULT  NOW(),
    updated_at TIMESTAMPTz NOT NULL DEFAULT  NOW(),
    PRIMARY KEY (id)
);

SELECT diesel_manage_updated_at('orders');