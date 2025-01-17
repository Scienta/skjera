use axum::extract::FromRequestParts;
use http::request::Parts;
use http::StatusCode;
use oauth2::{CsrfToken, PkceCodeVerifier};
use openidconnect::Nonce;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use crate::model::EmployeeId;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SkjeraSessionData {
    pub(crate) employee: EmployeeId,
    pub(crate) email: String,
    pub(crate) name: String,
    pub(crate) slack_connect: Option<SlackConnectData>,
}

impl SkjeraSessionData {
    pub(crate) fn mark_logged_in(
        self,
        employee: EmployeeId,
        email: String,
        name: String,
    ) -> SkjeraSessionData {
        SkjeraSessionData {
            employee,
            email,
            name,
            ..self
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SlackConnectData {
    csrf_token: CsrfToken,
    pub(crate) nonce: Nonce,
    pub(crate) pkce_verifier: String,
}

impl SkjeraSessionData {
    pub(crate) fn authenticated(&self) -> bool {
        self.employee.0 > 0
    }

    pub(crate) fn with_slack_connect(
        self,
        csrf_token: CsrfToken,
        nonce: Nonce,
        pkce_verifier: PkceCodeVerifier,
    ) -> Self {
        SkjeraSessionData {
            slack_connect: Some(SlackConnectData {
                csrf_token,
                nonce,
                pkce_verifier: pkce_verifier.secret().to_string(),
            }),
            ..self
        }
    }
}

impl Default for SkjeraSessionData {
    fn default() -> Self {
        Self {
            employee: EmployeeId(-1),
            email: "".to_string(),
            name: "".to_string(),
            slack_connect: None,
        }
    }
}

pub(crate) struct SkjeraSession {
    session: Session,
    pub(crate) data: SkjeraSessionData,
}

impl SkjeraSession {
    const SESSION_KEY: &'static str = "skjera";

    pub(crate) async fn mark_logged_in(
        &mut self,
        employee: EmployeeId,
        email: String,
        name: String,
    ) -> anyhow::Result<()> {
        let data = self.data.clone().mark_logged_in(employee, email, name);

        Self::update_session(&self, &data).await
        // self.update_session(&data).await
    }

    pub(crate) async fn with_slack_connect(
        &mut self,
        csrf_token: CsrfToken,
        nonce: Nonce,
        pkce_verifier: PkceCodeVerifier,
    ) -> anyhow::Result<()> {
        let data = self
            .data
            .clone()
            .with_slack_connect(csrf_token, nonce, pkce_verifier);

        Self::update_session(&self, &data).await
    }

    async fn update_session(
        session: &SkjeraSession,
        data: &SkjeraSessionData,
    ) -> anyhow::Result<()> {
        session
            .session
            .insert(Self::SESSION_KEY, data.clone())
            .await
            .map_err(|_| anyhow::anyhow!("Failed to insert session"))
    }
}

impl<S> FromRequestParts<S> for SkjeraSession
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(req: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(req, state).await?;

        let data: SkjeraSessionData = session
            .get(Self::SESSION_KEY)
            .await
            .unwrap()
            .unwrap_or_default();

        Ok(Self { session, data })
    }
}

impl<S> FromRequestParts<S> for SkjeraSessionData
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(req: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = SkjeraSession::from_request_parts(req, state).await?;

        Ok(session.data)
    }
}
