use askama::Template;
use async_trait::async_trait;
use axum::extract::Host;
use axum::http::Method;
use axum_extra::extract::CookieJar;
use skjera_api::apis::html::{HelloWorldResponse, Html};
use crate::ServerImpl;

#[derive(Template)]
#[template(path = "hello.html")]
pub(crate) struct HelloTemplate<'a> {
    pub name: &'a str,
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
        let template = HelloTemplate { name: "world" };

        match template.render() {
            Ok(text) => Ok(HelloWorldResponse::Status200_HelloWorld(text)),
            Err(e) => Err(e.to_string()),
        }
    }
}
