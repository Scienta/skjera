use crate::ServerImpl;
use askama_axum::Template;
use async_trait::async_trait;
use axum::extract::Host;
use axum::http::Method;
use axum_extra::extract::CookieJar;
use oauth2::reqwest::async_http_client;
use oauth2::{AuthorizationCode, TokenResponse};
use skjera_api::apis::html::{HelloWorldResponse, Html, OauthGoogleResponse};
use skjera_api::models::OauthGoogleQueryParams;
use url;

#[derive(Template)]
#[template(path = "hello.html"/*, print = "all"*/)]
pub(crate) struct HelloTemplate {
    pub name: String,
    pub google_auth_url: Option<String>,
}

#[allow(unused_variables)]
#[async_trait]
impl Html for ServerImpl {
    async fn hello_world(
        &self,
        method: Method,
        host: Host,
        cookies: CookieJar,
    ) -> Result<HelloWorldResponse, String> {
        let scope = "openid profile email";
        let url = url::Url::parse_with_params(
            "https://accounts.google.com/o/oauth2/v2/auth",
            &[
                ("scope", scope),
                ("client_id", &self.cfg.client_id),
                ("response_type", "code"),
                ("redirect_uri", &self.cfg.redirect_url),
            ],
        );

        if let Err(e) = url {
            return Err(e.to_string());
        }

        let url = url.unwrap();

        let template = HelloTemplate {
            name: "world".to_string(),
            google_auth_url: Some(url.to_string()),
        };

        match template.render() {
            Ok(text) => Ok(HelloWorldResponse::Status200_HelloWorld(text)),
            Err(e) => Err(e.to_string()),
        }
    }

    async fn oauth_google(
        &self,
        method: Method,
        host: Host,
        cookies: CookieJar,
        query_params: OauthGoogleQueryParams,
    ) -> Result<OauthGoogleResponse, String> {
        let code = query_params.code.unwrap_or_default();
        println!("code: {}", code);

        let token = self
            .basic_client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(async_http_client)
            .await;

        if let Err(e) = token {
            return Err(e.to_string());
        }
        let token = token.unwrap();

        println!("token: {:?}", token.scopes());

        let profile = self
            .ctx
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(token.access_token().secret().to_owned())
            .send()
            .await;

        if let Err(e) = profile {
            return Err(e.to_string());
        }
        let profile = profile.unwrap();

        // let profile_response = profile.text().await.unwrap();
        // println!("UserProfile: {:?}", profile_response);
        // let user_profile = serde_json::from_str::<UserProfile>(&profile_response).unwrap();

        let user_profile = profile.json::<UserProfile>().await.unwrap();

        println!("UserProfile: {:?}", user_profile);

        let template = HelloTemplate {
            name: user_profile.name,
            google_auth_url: None,
        };

        match template.render() {
            Ok(text) => Ok(OauthGoogleResponse::Status200_OAuthResponsesForGoogle(text)),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct UserProfile {
    sub: String,
    email: String,
    name: String,
    // given_name: Option<String>,
    // family_name: Option<String>,
}
