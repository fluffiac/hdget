use std::io;

// replace with actual webhook url
// this is meant to be a secret
const URL: &str = "fuckin shit";

/// wrapper over this weird library
pub struct Hook {
    client: webhook::client::WebhookClient,
}

impl Hook {
    pub fn new() -> Self {
        Self {
            client: webhook::client::WebhookClient::new(URL),
        }
    }

    /// send text to the discord webhook
    pub async fn send(&self, content: &str) -> io::Result<bool> {
        // dumb ratelimit fix
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        self.client
            .send(|mesg| mesg.content(content))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}
