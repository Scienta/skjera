use crate::ServerImpl;
use askama_axum::Template;
use async_trait::async_trait;
use axum::extract::Host;
use axum::http::Method;
use axum_extra::extract::CookieJar;
use skjera_api::apis::html::{HelloWorldResponse, Html};
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
}

#[derive(Debug, serde::Deserialize)]
pub struct UserProfile {
    sub: String,
    email: String,
    name: String,
    // given_name: Option<String>,
    // family_name: Option<String>,
}
