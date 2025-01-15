use anyhow::anyhow;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, info_span, instrument};

#[derive(Debug, Deserialize, Serialize)]
pub struct SlackResponse {
    pub ok: bool,
    pub profile: Option<SlackUserProfile>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SlackUserProfile {
    pub display_name: String,
}

#[instrument(skip_all)]
pub(crate) async fn user_profile_get(
    http_client: &Client,
    token: &String,
) -> anyhow::Result<SlackUserProfile> {
    let response = info_span!("users.profile.get")
        .in_scope(|| async {
            http_client
                .get("https://slack.com/api/users.profile.get")
                .bearer_auth(token)
                .send()
                .await
        })
        .await?;

    if response.status() != 200 {
        return Err(anyhow!("non-200 response"));
    }

    let response = {
        let response = response.text().await?;
        info!("response: {:?}", response);
        serde_json::from_str::<SlackResponse>(&response)?
    };

    if !response.ok {
        return Err(anyhow!("non-ok response"));
    }

    let response = response.profile.ok_or_else(|| anyhow!("bad response"))?;

    Ok(response)
}
