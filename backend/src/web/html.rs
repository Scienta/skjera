use crate::model::*;
use crate::session::SkjeraSessionData;
use crate::{AppError, AuthSession, ServerImpl};
use anyhow::{anyhow, Context};
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use once_cell::sync::Lazy;
use serde::Deserialize;
use time::{format_description, Date, Month};
use tracing::{debug, info, instrument, span, Level};
use url::Url;

static MONTH_NAMES: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        "January".to_string(),
        "February".to_string(),
        "March".to_string(),
        "April".to_string(),
        "May".to_string(),
        "June".to_string(),
        "July".to_string(),
        "August".to_string(),
        "September".to_string(),
        "October".to_string(),
        "November".to_string(),
        "December".to_string(),
    ]
});

#[derive(Template)]
#[template(path = "me.html")]
struct MeTemplate<'a> {
    pub month_names: &'a [String],
    pub days: Vec<i32>,
    pub dob_year: usize,
    pub dob_month: usize,
    pub dob_day: usize,
    pub some_accounts: Vec<SomeAccount>,

    pub slack_url: Option<String>,
}

#[axum::debug_handler]
#[tracing::instrument(skip(app, session))]
pub async fn get_me(
    State(app): State<ServerImpl>,
    session: AuthSession,
) -> Result<Html<String>, AppError> {
    let user = session.user.unwrap();

    let me = app
        .employee_dao
        .employee_by_email(user.email.clone())
        .await
        .map_err(AppError::Sqlx)?
        .ok_or_else(|| anyhow!("No such user: {}", user.email))
        .map_err(AppError::Anyhow)?;

    let some_accounts = app
        .employee_dao
        .some_accounts_by_employee(me.id)
        .await
        .map_err(AppError::Sqlx)?;

    // let slack_url = app.slack_connect
    //     .and_then(|slack_connect| slack_connect.slack_url().ok());
    let slack_url = app.slack_connect.map(|_| "/oauth/slack-begin".to_string());

    let template = MeTemplate {
        month_names: MONTH_NAMES.as_slice(),
        days: (1..31).collect::<Vec<i32>>(),
        dob_year: me.dob.map(|d| d.year() as usize).unwrap_or_default(),
        dob_month: me.dob.map(|d| d.month() as usize).unwrap_or_default(),
        dob_day: me.dob.map(|d| d.day() as usize).unwrap_or_default(),
        some_accounts,
        slack_url,
    };

    Ok(Html(template.render()?))
}

#[derive(Deserialize, Debug)]
pub(crate) struct MeForm {
    dob_year: i32,
    dob_month: u8,
    dob_day: u8,
}

pub async fn post_me(
    State(app): State<ServerImpl>,
    session: AuthSession,
    Form(input): Form<MeForm>,
) -> Result<Redirect, AppError> {
    let _span = span!(Level::INFO, "post_me");

    let user = session.user.unwrap();

    debug!("form: {:?}", input);

    let mut me = app
        .employee_dao
        .employee_by_email(user.email)
        .await?
        .context("error loading me")?;

    let year = input.dob_year;
    let month: Option<Month> = input.dob_month.try_into().ok();

    let dob: Option<Date> = match (month, input.dob_day) {
        (Some(m), d) if d >= 1 => Date::from_calendar_date(year, m, d).ok(),
        _ => None,
    };

    me.dob = dob;

    me = app.employee_dao.update(&me).await?;

    debug!("Updated Employee: {:?}", me);

    Ok(Redirect::to("/"))
}

pub async fn delete_some_account(
    State(app): State<ServerImpl>,
    session: AuthSession,
    Path(some_account_id): Path<SomeAccountId>,
) -> Result<Redirect, AppError> {
    let user = session.user.unwrap();

    info!(
        "some_account_id" = some_account_id.0,
        "Deleting some account"
    );

    app.employee_dao
        .delete_some_account(some_account_id, user.employee)
        .await?;

    Ok(Redirect::to("/me"))
}

#[derive(Deserialize, Debug)]
pub(crate) struct AddSomeAccountForm {
    bluesky: String,
    button_bluesky: Option<String>,

    linkedin: String,
    button_linkedin: Option<String>,

    x: String,
    button_x: Option<String>,
}

pub async fn add_some_account(
    State(app): State<ServerImpl>,
    session: AuthSession,
    Form(input): Form<AddSomeAccountForm>,
) -> Result<Redirect, AppError> {
    let _span = span!(Level::INFO, "add_some_account");

    let user = session.user.unwrap();

    info!("input" = ?input, "Adding some account");

    let mut network: Option<SomeNetwork> = None;
    let mut subject: Option<String> = None;
    let mut nick: Option<String> = None;
    let mut url: Option<String> = None;

    if input.button_bluesky.is_some() {
        network = Some(BLUESKY.to_owned());
        nick = Some(input.bluesky.clone());
        subject = nick.clone();
        url = Some(format!("https://bsky.app/profile/{}", input.bluesky.clone()).to_string());
    } else if input.button_linkedin.is_some() {
        network = Some(LINKED_IN.to_owned());
        url = Some(input.linkedin);
    } else if input.button_x.is_some() && input.x.trim().len() > 0 {
        network = Some(X.to_owned());
        nick = Some(input.x.clone());
        subject = nick.clone();
        url = Some(format!("https://x.com/{}", input.x.clone()).to_string());
    }

    if let Some(network) = network {
        let instance = None;
        let name = None;
        let avatar = None;
        let authenticated = false;

        app.employee_dao
            .add_some_account(
                user.employee,
                network,
                instance,
                authenticated,
                None,
                subject,
                name,
                nick,
                url,
                avatar,
            )
            .await?;
    }

    Ok(Redirect::to("/me"))
}

#[derive(Template)]
#[template(path = "employee.html")]
struct EmployeeTemplate {
    employee: Employee,
    some_accounts: Vec<SomeAccount>,
}

impl EmployeeTemplate {
    pub fn dob(&self) -> String {
        let f = format_description::parse("[year]-[month]-[day]")
            .ok()
            .unwrap();

        match self.employee.dob {
            Some(dob) => dob.format(&f).ok().unwrap_or_default(),
            _ => "".to_string(),
        }
    }
}

#[tracing::instrument(skip(app))]
pub async fn employee(
    State(app): State<ServerImpl>,
    Path(employee_id): Path<EmployeeId>,
) -> Result<Html<String>, AppError> {
    let employee = app
        .employee_dao
        .employee_by_id(employee_id)
        .await?
        .context("error loading me")?;

    let some_accounts = app
        .employee_dao
        .some_accounts_by_employee(employee_id)
        .await?;

    let template = EmployeeTemplate {
        employee,
        some_accounts,
    };

    Ok(Html(template.render()?))
}

#[tracing::instrument(skip(app))]
pub async fn employee_create_message(
    State(app): State<ServerImpl>,
    Path(employee_id): Path<EmployeeId>,
) -> Result<Html<String>, AppError> {
    let employee = app
        .employee_dao
        .employee_by_id(employee_id)
        .await?
        .context("error loading employee")?;

    let birthday_bot = app
        .birthday_bot
        .ok_or(anyhow!("birthday bot not configured"))?;

    let message = birthday_bot.create_message(&employee).await?;

    let template = EmployeeCreateMessageTemplate {
        employee,
        message: Some(message),
    };

    Ok(Html(template.render()?))
}

#[derive(Template)]
#[template(path = "employee-message.html")]
struct EmployeeCreateMessageTemplate {
    employee: Employee,
    message: Option<String>,
}

#[derive(Template)]
#[template(path = "hello.html"/*, print = "all"*/)]
struct HelloTemplate {
    pub user: Option<SkjeraSessionData>,
    pub employees: Option<Vec<Employee>>,
}

pub async fn hello_world(
    State(app): State<ServerImpl>,
    session: AuthSession,
) -> Result<Response, AppError> {
    let _span = span!(Level::INFO, "hello_world");

    if session.user.is_none() {
        return Ok(Redirect::to(crate::LOGIN_PATH).into_response());
    }

    let mut employees = None::<Vec<Employee>>;
    if session.user.is_some() {
        employees = Some(app.employee_dao.employees().await?);
    }

    let template = HelloTemplate {
        user: session.user,
        employees,
    };

    Ok(Html(template.render()?).into_response())
}

#[derive(Template)]
#[template(path = "login.html")]
pub(crate) struct LoginTemplate {
    pub google_auth_url: String,
}

#[instrument(skip(app, session))]
pub async fn login(
    State(app): State<ServerImpl>,
    mut session: AuthSession,
) -> Result<Html<String>, AppError> {
    let _ = session.logout().await;

    let scope = "openid profile email";

    let u = Url::parse_with_params(
        "https://accounts.google.com/o/oauth2/v2/auth",
        &[
            ("scope", scope),
            ("client_id", &app.cfg.client_id),
            ("response_type", "code"),
            ("redirect_uri", &app.cfg.redirect_url),
        ],
    )?;

    let template = LoginTemplate {
        google_auth_url: u.to_string(),
    };

    Ok(Html(template.render()?))
}

#[instrument(skip(session))]
pub async fn logout(
    mut session: AuthSession,
) -> Result<Redirect, AppError> {
    let _ = session
        .logout()
        .await
        .map_err(|e| anyhow!("Could not log out: {}", e))?;

    Ok(Redirect::to("/"))
}

#[derive(Template)]
#[template(path = "unauthorized.html")]
pub(crate) struct UnauthorizedTemplate {}
