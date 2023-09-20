// @generated automatically by Diesel CLI.

diesel::table! {
    employees (id) {
        id -> Int8,
        name -> Varchar,
        mobile_number -> Nullable<Varchar>,
        email -> Varchar,
        password -> Bpchar,
        last_login -> Timestamptz,
        invited_by -> Int8,
        role -> Int2,
        create_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    sys_files (id) {
        id -> Int8,
        hash -> Varchar,
        path -> Varchar,
        size -> Int8,
        is_video -> Nullable<Bool>,
        transcode_from -> Nullable<Int8>,
        can_be_encode -> Nullable<Bool>,
        slice_count -> Nullable<Int4>,
        bit_rate -> Nullable<Int4>,
        duration_ms -> Nullable<Int4>,
        height -> Nullable<Int4>,
        width -> Nullable<Int4>,
        general_info -> Nullable<Text>,
        video_info -> Nullable<Text>,
        audio_info -> Nullable<Text>,
        create_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    user_files (id) {
        id -> Int8,
        sys_file_id -> Nullable<Int8>,
        user_id -> Int8,
        parent_id -> Nullable<Int8>,
        at_dir -> Varchar,
        file_name -> Varchar,
        is_dir -> Bool,
        deleted -> Bool,
        create_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Int8,
        name -> Varchar,
        mobile_number -> Nullable<Varchar>,
        email -> Varchar,
        address -> Nullable<Varchar>,
        password -> Bpchar,
        last_login -> Timestamptz,
        create_at -> Timestamptz,
        updated_at -> Timestamptz,
        online -> Bool,
    }
}

diesel::allow_tables_to_appear_in_same_query!(employees, sys_files, user_files, users,);
