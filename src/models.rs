use crate::schema::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Queryable, Identifiable)]
#[table_name = "collections"]
pub struct Collection {
    pub id: i32,
    pub namespace: String,
    pub name: String,
}
#[derive(Debug, Insertable)]
#[table_name = "collections"]
pub struct CollectionNew<'a> {
    pub namespace: &'a str,
    pub name: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionJson {
    pub namespace: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Identifiable, Associations)]
#[table_name = "collection_versions"]
pub struct CollectionVersion {
    pub id: i32,
    pub collection_id: i32,
    pub version: String,
}

#[derive(Debug, Insertable)]
#[table_name = "collection_versions"]
pub struct CollectionVersionNew<'a> {
    pub collection_id: &'a i32,
    pub version: &'a str,
}
