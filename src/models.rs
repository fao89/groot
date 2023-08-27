use crate::schema::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = collections)]
pub struct Collection {
    pub id: i32,
    pub namespace: String,
    pub name: String,
}
#[derive(Debug, Insertable)]
#[diesel(table_name = collections)]
pub struct CollectionNew<'a> {
    pub namespace: &'a str,
    pub name: &'a str,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, Identifiable, Associations)]
#[diesel(belongs_to(Collection))]
#[diesel(table_name = collection_versions)]
pub struct CollectionVersion {
    pub id: i32,
    pub collection_id: i32,
    pub artifact: Value,
    pub version: String,
    pub metadata: Value,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = collection_versions)]
pub struct CollectionVersionNew<'a> {
    pub collection_id: &'a i32,
    pub artifact: &'a Value,
    pub version: &'a str,
    pub metadata: &'a Value,
}
