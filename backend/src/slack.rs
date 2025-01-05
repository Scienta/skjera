use crate::oauth::OauthResponse;
use crate::session::{SkjeraSession, SkjeraSessionData, SlackConnectData};
use crate::{model, AppError, ServerImpl};
use anyhow::{anyhow, Result};
use axum::extract::{Query, State};
use axum::response::Redirect;
use oauth2::PkceCodeVerifier;
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreProviderMetadata, CoreUserInfoClaims,
};
use openidconnect::reqwest::async_http_client;
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    PkceCodeChallenge, RedirectUrl, Scope,
};
use openidconnect::{OAuth2TokenResponse, TokenResponse};
use std::fmt::Debug;
use tracing::{debug, info, span, Level};
use url::Url;

#[derive(Clone, Debug)]
pub(crate) struct SlackConnect {
    client: CoreClient,
}

impl SlackConnect {
    pub(crate) async fn new(
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

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret)),
        )
        .set_redirect_uri(redirect_url.clone());

        Ok(SlackConnect { client })
    }

    // pub(crate) fn slack_url(self: &Self) -> Result<String> {
    //     let mut url = Url::parse("https://slack.com/openid/connect/authorize")?;
    //
    //     url.query_pairs_mut()
    //         .append_pair("scope", "openid email profile")
    //         .append_pair("response_type", "code")
    //         .append_pair("client_id", self.client_id.as_str())
    //         .append_pair("redirect_uri", self.redirect_url.as_str());
    //
    //     Ok(url.to_string())
    // }

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

    async fn slack_connect_continue(
        self: &Self,
        session: SlackConnectData,
        code: String,
    ) -> Result<CoreUserInfoClaims> {
        let pkce_verifier = PkceCodeVerifier::new(session.pkce_verifier);

        let token_response = span!(Level::INFO, "slack_connect", function = "exchange_token")
            .in_scope(|| async {
                self.client
                    .exchange_code(AuthorizationCode::new(code))
                    .set_pkce_verifier(pkce_verifier)
                    .request_async(async_http_client)
                    .await
            })
            .await?;

        // Extract the ID token claims after verifying its authenticity and nonce.
        let id_token = token_response
            .id_token()
            .ok_or_else(|| anyhow!("Server did not return an ID token"))?;

        debug!("Got Slack token");

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
        let user_info = span!(Level::INFO, "slack_connect", function = "user_info")
            .in_scope(|| async {
                self.client
                    .user_info(token_response.access_token().to_owned(), None)
                    .map_err(|err| anyhow!("No user info endpoint: {:?}", err))?
                    .request_async(async_http_client)
                    .await
                    .map_err(|err| anyhow!("Failed requesting user info: {:?}", err))
            })
            .await?;

        // See the OAuth2TokenResponse trait for a listing of other available fields such as
        // access_token() and refresh_token().

        info!("slack user info {:?}", user_info);

        Ok(user_info)
    }
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

    let user_info = slack_connect
        .slack_connect_continue(slack_connect_data, query.code)
        .await?;

    app.employee_dao
        .add_some_account(
            session.employee,
            model::SLACK.to_owned(),
            None,
            Some(user_info.subject().to_string()),
            user_info
                .name()
                .and_then(|x| x.get(None).map(|x| x.to_string())),
            user_info
                .nickname()
                .and_then(|x| x.get(None).map(|x| x.to_string())),
            None,
            user_info
                .picture()
                .and_then(|x| x.get(None).map(|x| x.to_string())),
        )
        .await?;

    Ok(Redirect::to("/"))
}
