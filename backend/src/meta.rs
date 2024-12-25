use async_trait::async_trait;
use axum::extract::Host;
use axum::http::Method;
use axum_extra::extract::CookieJar;
use skjera_api::apis::meta::{Meta, MetaHealthzResponse};
use crate::ServerImpl;

#[allow(unused_variables)]
#[async_trait]
impl Meta for ServerImpl {
    async fn meta_healthz(
        &self,
        method: Method,
        host: Host,
        cookies: CookieJar,
    ) -> Result<MetaHealthzResponse, String> {
        Ok(MetaHealthzResponse::Status200_Healthy)
    }
}
