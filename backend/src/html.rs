use crate::{AppError, ServerImpl, SessionUser};
use askama_axum::{Template};
use axum::extract::State;
use axum::response::Html;
use url;
use url::Url;

#[derive(Template)]
#[template(path = "hello.html"/*, print = "all"*/)]
struct HelloTemplate {
    pub name: String,
    pub google_auth_url: Option<String>,
}

pub async fn hello_world(
    State(app): State<ServerImpl>,
    user: Option<SessionUser>,
) -> Result<Html<String>, AppError> {
    let scope = "openid profile email";

    let mut name = "world".to_string();
    let mut url = None::<String>;
    if user.is_some() {
        let user = user.unwrap();
        name = user.email;
    } else {
        let u = Url::parse_with_params(
            "https://accounts.google.com/o/oauth2/v2/auth",
            &[
                ("scope", scope),
                ("client_id", &app.cfg.client_id),
                ("response_type", "code"),
                ("redirect_uri", &app.cfg.redirect_url),
            ],
        )?.to_string();
        url = Some(u)
    }

    let template = HelloTemplate {
        name,
        google_auth_url: url,
    };

    Ok(Html(template.render()?))
}
