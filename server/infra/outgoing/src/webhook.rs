use std::collections::HashMap;

pub struct Webhook {
    target_url: String,
    request_body: HashMap<String, String>,
}

impl Webhook {
    pub fn new(url: String) -> Self {
        Self {
            target_url: url,
            request_body: HashMap::new(),
        }
    }

    pub fn username(&self, name: String) -> Self {
        let mut body = self.request_body.to_owned();
        body.insert("username".to_string(), name);
        body.insert("content".to_string(), "test".to_string());

        Self {
            target_url: self.target_url.to_owned(),
            request_body: body,
        }
    }

    pub async fn send(&self) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        client
            .post(self.target_url.to_owned())
            .json(&self.request_body)
            .send()
            .await?;
        Ok(())
    }
}
