use crate::model::Employee;
use crate::{AppError, ServerImpl};
use axum::extract::{Query, State};
use axum::response::Redirect;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{AuthUrl, AuthorizationCode, ClientId, ClientSecret, RedirectUrl, TokenResponse, TokenUrl};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, span, Level};
use crate::session::SkjeraSession;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct OauthResponse {
    pub(crate) code: String,
}

pub(crate) async fn oauth_google(
    Query(query): Query<OauthResponse>,
    State(app): State<ServerImpl>,
    mut session: SkjeraSession,
) -> Result<Redirect, AppError> {
    let _method = span!(Level::INFO, "oauth_google_inner");

    let code = query.code;
    debug!("code: {}", code);

    let token = {
        let _span = span!(Level::DEBUG, "exchange_code");
        app.basic_client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(async_http_client)
            .await?
    };

    debug!("token: {:?}", token.scopes());

    let profile = {
        let _span = span!(Level::DEBUG, "userinfo");
        app.ctx
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(token.access_token().secret().to_owned())
            .send()
            .await?
    };

    // let profile_response = profile.text().await.unwrap();
    // println!("UserProfile: {:?}", profile_response);
    // let user_profile = serde_json::from_str::<UserProfile>(&profile_response).unwrap();

    let user_profile = profile.json::<GoogleUserProfile>().await?;
    info!("UserProfile: {:?}", user_profile);

    let employee = load_or_create_employee(&app, &user_profile).await?;

    session.mark_logged_in(employee.id, user_profile.email, user_profile.name).await?;

    Ok(Redirect::to("/"))
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
