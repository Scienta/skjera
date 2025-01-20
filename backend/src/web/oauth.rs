use crate::web::html::UnauthorizedTemplate;
use crate::model::Employee;
use crate::session::SkjeraSessionData;
use crate::{AppError, ServerImpl};
use anyhow::anyhow;
use askama_axum::Template;
use async_trait::async_trait;
use axum::extract::Query;
use axum::response::*;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_login::{AuthnBackend, UserId};
use http::StatusCode;
use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, RedirectUrl, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, span, Level};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct OauthResponse {
    pub(crate) code: String,
}

pub(crate) async fn oauth_google(
    Query(OauthResponse { code }): Query<OauthResponse>,
    mut auth_session: crate::AuthSession,
) -> Response {
    let _method = span!(Level::INFO, "oauth_google_inner");

    debug!("code: {}", code);

    let creds = SkjeraAuthnCredentials { code };

    let user = match auth_session.authenticate(creds).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let template = UnauthorizedTemplate {};

            return (StatusCode::UNAUTHORIZED, Html(template.render().unwrap())).into_response();
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    match auth_session.login(&user).await {
        Ok(()) => Redirect::to("/me").into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Anyhow(anyhow!("Could not log in user: {}: {}", user.email, e)),
        )
            .into_response(),
    }
}

async fn load_or_create_employee(
    app: &ServerImpl,
    user_profile: &GoogleUserProfile,
) -> Result<Employee, anyhow::Error> {
    let employee = app
        .employee_dao
        .employee_by_email(user_profile.email.clone())
        .await?;

    if let Some(e) = employee {
        info!("Loaded employee user: {:?}", e);
        return Ok(e);
    }

    let employee = app
        .employee_dao
        .insert_employee(user_profile.email.clone(), user_profile.name.clone())
        .await?;

    info!("Created new employee: {:?}", employee);

    Ok(employee)
}

pub(crate) fn build_oauth_client(
    redirect_url: String,
    client_id: String,
    client_secret: String,
) -> BasicClient {
    // If you're not using Google OAuth, you can use whatever the relevant auth/token URL is for your given OAuth service
    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())
        .expect("Invalid token endpoint URL");

    BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url).unwrap())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleUserProfile {
    sub: String,
    email: String,
    name: String,
    // given_name: Option<String>,
    // family_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkjeraAuthnCredentials {
    pub code: String,
    // pub old_state: CsrfToken,
    // pub new_state: CsrfToken,
}

impl ServerImpl {
    #[tracing::instrument(skip(self, creds))]
    async fn exchange_code(
        self: &Self,
        creds: SkjeraAuthnCredentials,
    ) -> anyhow::Result<BasicTokenResponse> {
        self.basic_client
            .exchange_code(AuthorizationCode::new(creds.code))
            .request_async(async_http_client)
            .await
            .map_err(|e| anyhow!("Error exchanging oauth code: {}", e))
    }

    #[tracing::instrument(skip(self, token))]
    async fn user_info(
        self: &Self,
        token: &BasicTokenResponse,
    ) -> anyhow::Result<reqwest::Response> {
        self.ctx
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(token.access_token().secret().to_owned())
            .send()
            .await
            .map_err(|e| anyhow!("Could not fetch userinfo: {}", e))
    }
}

#[async_trait]
impl AuthnBackend for ServerImpl {
    type User = SkjeraSessionData;
    type Credentials = SkjeraAuthnCredentials;
    type Error = AppError;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let token = self.exchange_code(creds).await?;

        debug!("token: {:?}", token.scopes());

        let profile = self.user_info(&token).await?;

        // let profile_response = profile.text().await.unwrap();
        // println!("UserProfile: {:?}", profile_response);
        // let user_profile = serde_json::from_str::<UserProfile>(&profile_response).unwrap();

        let user_profile = profile
            .json::<GoogleUserProfile>()
            .await
            .map_err(AppError::Reqwest)?;
        info!("UserProfile: {:?}", user_profile);

        let employee = load_or_create_employee(self, &user_profile).await?;

        // session
        //     .mark_logged_in(employee.id, user_profile.email, user_profile.name)
        //     .await?;

        Ok(Some(Self::session_data(employee)))
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        // TODO: fix this
        let user = self
            .employee_dao
            .employee_by_id(*user_id)
            .await?
            .map(Self::session_data);

        Ok(user)
    }
}
