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
pub enum MetaHealthzResponse {
    /// Healthy
    Status200_Healthy
    ,
    /// Unhealthy
    Status503_Unhealthy
}


/// Meta
#[async_trait]
#[allow(clippy::ptr_arg)]
pub trait Meta {
    /// MetaHealthz - GET /meta/healthz
    async fn meta_healthz(
    &self,
    method: Method,
    host: Host,
    cookies: CookieJar,
    ) -> Result<MetaHealthzResponse, String>;
}
