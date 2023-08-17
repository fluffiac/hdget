use std::io;

const URL: &str = "fuckin shit";

pub struct Hook {
    client: webhook::client::WebhookClient,
}

impl Hook {
    pub fn new() -> Self {
        Self {
            client: webhook::client::WebhookClient::new(URL),
        }
    }

    pub async fn send(&self, content: &str) -> io::Result<bool> {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        self.client
            .send(|mesg| mesg.content(content))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}
