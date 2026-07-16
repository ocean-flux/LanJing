//! Diesel 行模型：规则 / 媒体等表的 Queryable / Insertable 结构。

use diesel::{Insertable, Queryable, Selectable};

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::rules)]
pub(crate) struct RuleRow {
    pub(crate) id: String,
    pub(crate) graph_json: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::rules)]
pub(crate) struct NewRuleRow<'a> {
    pub(crate) id: &'a str,
    pub(crate) source_url: &'a str,
    pub(crate) graph_json: &'a str,
    pub(crate) import_hash: &'a str,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::media)]
pub(crate) struct MediaRow {
    pub(crate) id: String,
    pub(crate) media_json: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::media)]
pub(crate) struct NewMediaRow<'a> {
    pub(crate) id: &'a str,
    pub(crate) source_id: &'a str,
    pub(crate) media_json: &'a str,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::cookies)]
pub(crate) struct CookieRow {
    pub(crate) id: String,
    pub(crate) cookie_json: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::cookies)]
pub(crate) struct NewCookieRow<'a> {
    pub(crate) id: &'a str,
    pub(crate) domain: &'a str,
    pub(crate) cookie_json: &'a str,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::media_graph)]
pub(crate) struct MediaGraphRow {
    pub(crate) id: i32,
    pub(crate) delta_json: String,
    pub(crate) updated_at: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::media_graph)]
pub(crate) struct NewMediaGraphRow<'a> {
    pub(crate) id: i32,
    pub(crate) delta_json: &'a str,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::library_entries)]
pub(crate) struct LibraryEntryRow {
    pub(crate) resource_id: String,
    pub(crate) favorite: i32,
    pub(crate) pinned: i32,
    pub(crate) last_opened_at: Option<String>,
    pub(crate) progress_json: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::library_entries)]
pub(crate) struct NewLibraryEntryRow<'a> {
    pub(crate) resource_id: &'a str,
    pub(crate) favorite: i32,
    pub(crate) pinned: i32,
    pub(crate) last_opened_at: Option<&'a str>,
    pub(crate) progress_json: Option<&'a str>,
}
