use crate::web::oauth::oauth_google;
use crate::web::slack_bot::*;
use crate::web::{html, slack};
use crate::ServerImpl;
use anyhow::Result;
use axum::routing::{get, post};
use axum::Router;
use slack_morphism::prelude::*;
use std::sync::Arc;

pub(crate) fn create_router(app: &ServerImpl) -> Result<(Router<ServerImpl>, Router<ServerImpl>)> {
    let mut public = Router::new()
        .route("/", get(html::hello_world))
        .route("/login", get(html::login))
        .route("/logout", get(html::logout))
        .route("/oauth/google", get(oauth_google));

    if app.slack_client.is_some() {
        let slack: Router<ServerImpl> = create_slack(app)?;

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

fn create_slack(app: &ServerImpl) -> Result<Router<ServerImpl>> {
    let (slack_client, signing_secret) =
        match (app.slack_client.clone(), app.cfg.slack_config.clone()) {
            (Some(slack_client), Some(slack_config)) => {
                (slack_client, &slack_config.signing_secret.clone())
            }
            _ => return Err(anyhow::anyhow!("missing slack client")),
        };

    let listener_environment: Arc<SlackHyperListenerEnvironment> = Arc::new(
        SlackClientEventsListenerEnvironment::new(slack_client)
            .with_error_handler(slack_error_handler),
    );

    let listener: SlackEventsAxumListener<SlackHyperHttpsConnector> =
        SlackEventsAxumListener::new(listener_environment.clone());

    let router = Router::new()
        .route(
            "/api/slack-push",
            post(slack_push_event).layer(
                listener
                    .events_layer(&signing_secret)
                    .with_event_extractor(SlackEventsExtractors::push_event()),
            ),
        )
        .route(
            "/api/slack-interaction",
            post(slack_interaction_event).layer(
                listener
                    .events_layer(&signing_secret)
                    .with_event_extractor(SlackEventsExtractors::interaction_event()),
            ),
        );

    Ok(router)
}
