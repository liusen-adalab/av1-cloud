// @generated automatically by Diesel CLI.

diesel::table! {
    employees (id) {
        id -> Int8,
        name -> Varchar,
        mobile_number -> Nullable<Varchar>,
        email -> Varchar,
        password -> Bpchar,
        last_login -> Timestamp,
        invited_by -> Int8,
        role -> Int2,
        create_at -> Timestamp,
        updated_at -> Timestamp,
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
        last_login -> Timestamp,
        create_at -> Timestamp,
        updated_at -> Timestamp,
        online -> Bool,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    employees,
    users,
);
