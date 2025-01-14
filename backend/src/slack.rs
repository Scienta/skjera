use crate::oauth::OauthResponse;
use crate::session::{SkjeraSession, SkjeraSessionData, SlackConnectData};
use crate::{model, AppError, ServerImpl};
use anyhow::{anyhow, Result};
use axum::extract::{Query, State};
use axum::response::Redirect;
use oauth2::{HttpRequest, HttpResponse, PkceCodeVerifier};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreGenderClaim, CoreProviderMetadata,
};
use openidconnect::reqwest::async_http_client;
use openidconnect::{
    AccessTokenHash, AdditionalClaims, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, RedirectUrl, Scope, TokenResponse,
    UserInfoClaims,
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tracing::{debug, info, span, Level};
use url::Url;

type SlackUserInfoClaims = UserInfoClaims<SlackAdditionalClaims, CoreGenderClaim>;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SlackAdditionalClaims {
    #[serde(rename = "https://slack.com/team_id")]
    team_id: Option<String>,

    #[serde(rename = "https://slack.com/team_domain")]
    team_domain: Option<String>,

    #[serde(rename = "https://slack.com/team_image_230")]
    team_image_230: Option<String>,
}
impl AdditionalClaims for SlackAdditionalClaims {}

#[derive(Clone, Debug)]
pub(crate) struct SlackConnect {
    client: CoreClient,
    http_client: reqwest::Client,
}

impl SlackConnect {
    pub(crate) async fn new(
        http_client: reqwest::Client,
        client_id: String,
        client_secret: String,
        redirect_url: String,
    ) -> Result<SlackConnect> {
        let provider_metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new("https://slack.com".to_string())?,
            async_http_client,
        )
        .await?;

        let redirect_url = RedirectUrl::new(redirect_url.clone())?;

        // Create an OpenID Connect client by specifying the client ID, client secret, authorization URL
        // and token URL.

        // let client = SlackOauth2Client::from_provider_metadata(
        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret)),
        )
        .set_redirect_uri(redirect_url.clone());

        Ok(SlackConnect {
            http_client,
            client,
        })
    }

    async fn slack_connect_begin(self: &Self) -> Result<(Url, CsrfToken, Nonce, PkceCodeVerifier)> {
        // Generate a PKCE challenge.
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Generate the full authorization URL.
        let (auth_url, csrf_token, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            // Set the PKCE code challenge.
            .set_pkce_challenge(pkce_challenge)
            .url();

        Ok((auth_url, csrf_token, nonce, pkce_verifier))
    }

    //noinspection RsConstantConditionIf
    async fn slack_http_client(
        req: HttpRequest,
    ) -> Result<HttpResponse, oauth2::reqwest::Error<reqwest::Error>> {
        let result = async_http_client(req).await?;

        // Enable this to log http responses
        if false {
            let r = result.clone();

            let body = r.body;

            let json: serde_json::Value =
                serde_json::from_slice(body.as_slice()).expect("JSON was not well-formatted");

            info!("JSON {}", json);
        }

        Ok(result)
    }

    async fn slack_connect_continue(
        self: &Self,
        session: SlackConnectData,
        code: String,
    ) -> Result<(SlackUserInfoClaims, SlackUserProfile)> {
        let _span = span!(Level::INFO, "slack_connect_continue");
        let pkce_verifier = PkceCodeVerifier::new(session.pkce_verifier);

        let token_response = span!(Level::INFO, "exchange_token")
            .in_scope(|| async {
                self.client
                    .exchange_code(AuthorizationCode::new(code))
                    .set_pkce_verifier(pkce_verifier)
                    .request_async(&Self::slack_http_client)
                    .await
            })
            .await?;

        // Extract the ID token claims after verifying its authenticity and nonce.
        let id_token = token_response
            .id_token()
            .ok_or_else(|| anyhow!("Server did not return an ID token"))?;

        debug!("Got Slack token");
        debug!("Got Slack token: {}", id_token.to_string());

        let claims = id_token.claims(&self.client.id_token_verifier(), &session.nonce)?;

        info!("claims: {:?}", claims);

        // Verify the access token hash to ensure that the access token hasn't been substituted for
        // another user's.
        if let Some(expected_access_token_hash) = claims.access_token_hash() {
            let actual_access_token_hash = AccessTokenHash::from_token(
                token_response.access_token(),
                &id_token.signing_alg()?,
            )?;
            if actual_access_token_hash != *expected_access_token_hash {
                return Err(anyhow!("Invalid access token"));
            }
        }

        // If available, we can use the UserInfo endpoint to request additional information.

        // The user_info request uses the AccessToken returned in the token response. To parse custom
        // claims, use UserInfoClaims directly (with the desired type parameters) rather than using the
        // CoreUserInfoClaims type alias.
        let user_info = span!(Level::INFO, "user_info")
            .in_scope(|| async {
                self.client
                    .user_info(token_response.access_token().to_owned(), None)
                    .map_err(|err| anyhow!("No user info endpoint: {:?}", err))?
                    .request_async(&Self::slack_http_client)
                    .await
                    .map_err(|err| anyhow!("Failed requesting user info: {:?}", err))
            })
            .await?;

        // See the OAuth2TokenResponse trait for a listing of other available fields such as
        // access_token() and refresh_token().

        info!("slack user info {:?}", user_info);

        let response = span!(Level::INFO, "users.profile.get")
            .in_scope(|| async {
                self.http_client
                    .get("https://slack.com/api/users.profile.get")
                    .bearer_auth(token_response.access_token().secret())
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

        let user_profile = response.profile.ok_or_else(|| anyhow!("bad response"))?;

        info!("slack user profile {:?}", user_profile);

        Ok((user_info, user_profile))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SlackResponse {
    ok: bool,
    profile: Option<SlackUserProfile>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SlackUserProfile {
    display_name: String,
}

pub(crate) async fn oauth_slack_begin(
    State(app): State<ServerImpl>,
    mut session: SkjeraSession,
) -> std::result::Result<Redirect, AppError> {
    let _method = span!(Level::INFO, "oauth_slack_begin");

    let slack_connect = app
        .slack_connect
        .ok_or_else(|| anyhow!("slack not enabled"))?;

    let (auth_url, csrf_token, nonce, pkce_verifier) = slack_connect.slack_connect_begin().await?;

    info!(
        auth_url = auth_url.as_str(),
        // csrf_token = csrf_token.secret(),
        // nonce = nonce.secret(),
        // pkce_verifier = pkce_verifier.secret(),
        "Slack connect successful"
    );

    session
        .with_slack_connect(csrf_token, nonce, pkce_verifier)
        .await?;

    Ok(Redirect::to(auth_url.as_str()))
}

pub(crate) async fn oauth_slack(
    State(app): State<ServerImpl>,
    Query(query): Query<OauthResponse>,
    session: SkjeraSessionData,
) -> std::result::Result<Redirect, AppError> {
    let _method = span!(Level::INFO, "oauth_slack");

    let slack_connect = app
        .clone()
        .slack_connect
        .ok_or_else(|| anyhow!("slack not enabled"))?;

    let slack_connect_data = session
        .slack_connect
        .ok_or_else(|| anyhow!("Not in a oauth process"))?;

    let (user_info, user_profile) = slack_connect
        .slack_connect_continue(slack_connect_data, query.code)
        .await?;

    let _team_domain = user_info
        .additional_claims()
        .clone()
        .team_domain
        .map(|team_domain| format!("https://{}.slack.com", team_domain));

    let authenticated = true;

    let network = model::SLACK.to_owned();
    let network_instance = user_info.additional_claims().clone().team_id;
    let network_avatar = user_info.additional_claims().clone().team_image_230;
    let subject = Some(user_info.subject().to_string());
    let name = user_info
        .name()
        .and_then(|x| x.get(None).map(|x| x.to_string()));
    let nick = user_profile
        .display_name;
    let avatar = user_info
        .picture()
        .and_then(|x| x.get(None).map(|x| x.to_string()));

    let account = app
        .employee_dao
        .some_account_for_network(
            session.employee,
            network.to_string(),
            network_instance.clone(),
        )
        .await?;
    let account = match account {
        Some(account) => {
            info!(old_account = ?account, "Updating exising account");

            app.employee_dao
                .update_some_account(
                    account.id,
                    authenticated,
                    network_avatar,
                    subject,
                    name,
                    Some(nick),
                    None,
                    avatar,
                )
                .await?
        }
        None => {
            info!("Creating new account");

            app.employee_dao
                .add_some_account(
                    session.employee,
                    network.to_owned(),
                    network_instance,
                    authenticated,
                    network_avatar,
                    subject,
                    name,
                    Some(nick),
                    None,
                    avatar,
                )
                .await?
        }
    };

    info!(?account, "New/updated account");

    Ok(Redirect::to("/"))
}
