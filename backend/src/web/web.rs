use crate::web::oauth::oauth_google;
use crate::web::slack_bot::*;
use crate::web::{html, slack};
use crate::{Config, ServerImpl, SlackConfig};
use anyhow::Result;
use axum::routing::{get, post};
use axum::Router;
use slack_morphism::prelude::*;
use std::sync::Arc;

pub(crate) fn create_router(config: &Config) -> Result<(Router<ServerImpl>, Router<ServerImpl>)> {
    let mut public = Router::new()
        .route("/", get(html::hello_world))
        .route("/login", get(html::login))
        .route("/logout", get(html::logout))
        .route("/oauth/google", get(oauth_google));

    if let Some(slack_config) = config.slack_config.clone() {
        let slack: Router<ServerImpl> = create_slack(slack_config)?;

        public = public.merge(slack);
    }

    let private = Router::new()
        .route("/me", get(html::get_me))
        .route("/me", post(html::post_me))
        .route("/me/some_account/add", post(html::add_some_account))
        .route(
            "/me/some_account/{some_account_id}/delete",
            post(html::delete_some_account),
        )
        .route("/employee/{employee_id}", get(html::employee))
        .route(
            "/employee/{employee_id}/create-message",
            get(html::employee_create_message),
        )
        .route("/oauth/slack-begin", get(slack::oauth_slack_begin))
        .route("/oauth/slack", get(slack::oauth_slack));

    Ok((public, private))
}

fn create_slack(slack_config: SlackConfig) -> Result<Router<ServerImpl>> {
    let client: Arc<SlackHyperClient> =
        Arc::new(SlackClient::new(SlackClientHyperConnector::new()?));

    let listener_environment: Arc<SlackHyperListenerEnvironment> = Arc::new(
        SlackClientEventsListenerEnvironment::new(client.clone())
            .with_error_handler(slack_error_handler),
    );

    let listener: SlackEventsAxumListener<SlackHyperHttpsConnector> =
        SlackEventsAxumListener::new(listener_environment.clone());

    let router = Router::new().route(
        "/api/slack-push",
        post(slack_push_event).layer(
            listener
                .events_layer(&slack_config.signing_secret)
                .with_event_extractor(SlackEventsExtractors::push_event()),
        ),
    );

    Ok(router)
}
