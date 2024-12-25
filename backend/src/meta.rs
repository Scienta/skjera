use crate::ServerImpl;
use async_trait::async_trait;
use axum::extract::Host;
use axum::http::Method;
use axum_extra::extract::CookieJar;
use skjera_api::apis::meta::{Meta, MetaHealthzResponse};

#[allow(unused_variables)]
#[async_trait]
impl Meta for ServerImpl {
    async fn meta_healthz(
        &self,
        method: Method,
        host: Host,
        cookies: CookieJar,
    ) -> Result<MetaHealthzResponse, String> {
        sqlx::query!("SELECT 1 AS junk")
            .fetch_all(&self.pool)
            .await
            .map(|_r| MetaHealthzResponse::Status200_Healthy)
            .map_err(|e| e.to_string()) // TODO: this can probably be done better somewhere else
    }
}
