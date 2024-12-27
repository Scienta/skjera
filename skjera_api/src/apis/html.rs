use async_trait::async_trait;
use axum::extract::*;
use axum_extra::extract::{CookieJar, Multipart};
use bytes::Bytes;
use http::Method;
use serde::{Deserialize, Serialize};

use crate::{models, types::*};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
#[allow(clippy::large_enum_variant)]
pub enum HelloWorldResponse {
    /// Hello World
    Status200_HelloWorld
    (String)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
#[allow(clippy::large_enum_variant)]
pub enum OauthGoogleResponse {
    /// OAuth responses for Google
    Status200_OAuthResponsesForGoogle
    (String)
}


/// Html
#[async_trait]
#[allow(clippy::ptr_arg)]
pub trait Html {
    /// HelloWorld - GET /
    async fn hello_world(
    &self,
    method: Method,
    host: Host,
    cookies: CookieJar,
    ) -> Result<HelloWorldResponse, String>;

    /// OauthGoogle - GET /oauth/google
    async fn oauth_google(
    &self,
    method: Method,
    host: Host,
    cookies: CookieJar,
      query_params: models::OauthGoogleQueryParams,
    ) -> Result<OauthGoogleResponse, String>;
}
