use crate::model::EmployeeId;
use axum_login::AuthUser;
use oauth2::{CsrfToken, PkceCodeVerifier};
use openidconnect::Nonce;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SkjeraSessionData {
    pub(crate) employee: EmployeeId,
    pub(crate) session_hash: Box<[u8]>,
    pub(crate) email: String,
    pub(crate) name: String,
    pub(crate) slack_connect: Option<SlackConnectData>,
}

impl AuthUser for SkjeraSessionData {
    type Id = EmployeeId;

    fn id(&self) -> Self::Id {
        self.employee
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.session_hash.as_ref()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SlackConnectData {
    csrf_token: CsrfToken,
    pub(crate) nonce: Nonce,
    pub(crate) pkce_verifier: String,
}

impl SkjeraSessionData {
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
