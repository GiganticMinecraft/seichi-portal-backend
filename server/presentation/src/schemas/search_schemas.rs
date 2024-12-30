use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq)]
pub struct SearchQuery {
    #[serde(default)]
    pub query: Option<String>,
}
