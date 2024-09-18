use errors::infra::InfraError;
use itertools::Itertools;
use serde::Serialize;
use serde_json::json;

#[derive(Debug)]
pub struct Webhook {
    target_url: String,
    title: String,
    fields: Vec<Field>,
}

#[derive(Debug, Serialize)]
struct SendContents {
    username: String,
    embeds: Vec<Embeds>,
}

#[derive(Debug, Serialize)]
struct Embeds {
    title: String,
    color: i32,
    fields: Vec<Field>,
}

#[derive(Debug, Clone, Serialize)]
struct Field {
    name: String,
    value: String,
    inline: bool,
}

#[derive(Debug)]
pub enum Color {
    Red,
    Lime,
    Aqua,
}

impl Color {
    pub fn to_color_code(&self) -> i32 {
        match self {
            Color::Red => 16711680,
            Color::Lime => 65280,
            Color::Aqua => 65535,
        }
    }
}

impl Webhook {
    #[tracing::instrument]
    pub fn new(url: String, title: String) -> Self {
        Self {
            target_url: url,
            title,
            fields: vec![],
        }
    }

    #[tracing::instrument]
    pub fn field(&self, name: String, value: String, inline: bool) -> Self {
        let field = Field {
            name,
            value,
            inline,
        };

        Self {
            target_url: self.target_url.to_owned(),
            title: self.title.to_owned(),
            fields: vec![self.fields.to_vec(), vec![field]]
                .into_iter()
                .flatten()
                .collect(),
        }
    }

    #[tracing::instrument]
    pub fn fields(&self, name_and_values: Vec<(String, String)>, inline: bool) -> Self {
        let fields = name_and_values
            .into_iter()
            .map(|(name, value)| Field {
                name,
                value,
                inline,
            })
            .collect_vec();

        Self {
            target_url: self.target_url.to_owned(),
            title: self.title.to_owned(),
            fields: vec![self.fields.to_vec(), fields]
                .into_iter()
                .flatten()
                .collect(),
        }
    }

    #[tracing::instrument]
    pub async fn send(&self, color: Color) -> Result<(), InfraError> {
        let contents = SendContents {
            username: "seichi-portal".to_string(),
            embeds: vec![Embeds {
                title: self.title.to_owned(),
                color: color.to_color_code(),
                fields: self.fields.to_vec(),
            }],
        };

        let client = reqwest::Client::new();
        client
            .post(self.target_url.to_owned())
            .json(&json!(contents))
            .send()
            .await
            .map_err(|cause| InfraError::Outgoing {
                cause: cause.to_string(),
            })?;
        Ok(())
    }
}
