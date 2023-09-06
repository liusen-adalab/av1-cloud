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

diesel::allow_tables_to_appear_in_same_query!(
    employees,
    users,
);
