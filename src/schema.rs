table! {
    collection_versions (id) {
        id -> Int4,
        collection_id -> Int4,
        version -> Varchar,
        metadata -> Json,
    }
}

table! {
    collections (id) {
        id -> Int4,
        namespace -> Varchar,
        name -> Varchar,
    }
}

joinable!(collection_versions -> collections (collection_id));

allow_tables_to_appear_in_same_query!(collection_versions, collections,);
