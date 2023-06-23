use serde::Serialize;
use serde_json::json;

pub struct Webhook {
    target_url: String,
    title: String,
    fields: Vec<Field>,
}

#[derive(Serialize)]
struct SendContents {
    username: String,
    embeds: Vec<Embeds>,
}

#[derive(Serialize)]
struct Embeds {
    title: String,
    fields: Vec<Field>,
}

#[derive(Clone, Serialize)]
struct Field {
    name: String,
    value: String,
    inline: bool,
}

impl Webhook {
    pub fn new(url: String, title: String) -> Self {
        Self {
            target_url: url,
            title,
            fields: vec![],
        }
    }

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

    pub async fn send(&self) -> anyhow::Result<()> {
        let contents = SendContents {
            username: "seichi-portal-backend".to_string(),
            embeds: vec![Embeds {
                title: self.title.to_owned(),
                fields: self.fields.to_vec(),
            }],
        };

        let client = reqwest::Client::new();
        client
            .post(self.target_url.to_owned())
            .json(&json!(contents))
            .send()
            .await?;
        Ok(())
    }
}
