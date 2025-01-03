use crate::model::*;
use crate::{AppError, ServerImpl, SessionUser};
use anyhow::Context;
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::{Html, Redirect};
use axum::Form;
use once_cell::sync::Lazy;
use serde::Deserialize;
use time::{format_description, Date, Month};
use tracing::{debug, info, span, Level};
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
    pub dob_month: usize,
    pub dob_day: usize,
    pub some_accounts: Vec<SomeAccount>,
}

#[tracing::instrument]
pub async fn get_me(
    State(app): State<ServerImpl>,
    user: SessionUser,
) -> Result<Html<String>, AppError> {
    let me = app
        .employee_dao
        .employee_by_email(user.email)
        .await?
        .context("error loading me")?;

    let some_accounts = app.employee_dao.some_accounts_by_employee(me.id).await?;

    let template = MeTemplate {
        month_names: MONTH_NAMES.as_slice(),
        days: (1..31).collect::<Vec<i32>>(),
        dob_month: me.dob.map(|d| d.month() as usize).unwrap_or_default(),
        dob_day: me.dob.map(|d| d.day() as usize).unwrap_or_default(),
        some_accounts,
    };

    Ok(Html(template.render()?))
}

#[derive(Deserialize, Debug)]
pub(crate) struct MeForm {
    dob_month: u8,
    dob_day: u8,
}

#[tracing::instrument]
pub async fn post_me(
    State(app): State<ServerImpl>,
    user: SessionUser,
    Form(input): Form<MeForm>,
) -> Result<Redirect, AppError> {
    debug!("form: {:?}", input);

    let mut me = app
        .employee_dao
        .employee_by_email(user.email)
        .await?
        .context("error loading me")?;

    let year = me.dob.map(|dob| dob.year()).unwrap_or_else(|| 1900);
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
    user: SessionUser,
    Path(some_account_id): Path<SomeAccountId>,
) -> Result<Redirect, AppError> {
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
    user: SessionUser,
    Form(input): Form<AddSomeAccountForm>,
) -> Result<Redirect, AppError> {
    let _span = span!(Level::INFO, "add_some_account");

    info!("input" = ?input, "Adding some account");

    let mut network: Option<String> = None;
    let mut nick: Option<String> = None;
    let mut url: Option<String> = None;

    if input.button_bluesky.is_some() {
        network = Some("bluesky".to_string());
        nick = Some(input.bluesky.clone());
        url = Some(format!("https://bsky.app/profile/{}", input.bluesky.clone()).to_string());
    } else if input.button_linkedin.is_some() {
        network = Some("linkedin".to_string());
        url = Some(input.linkedin);
    } else if input.button_x.is_some() && input.x.trim().len() > 0 {
        network = Some("x".to_string());
        nick = Some(input.x.clone());
        url = Some(format!("https://x.com/{}", input.x.clone()).to_string());
    }

    if let Some(network) = network {
        app.employee_dao
            .add_some_account(user.employee, network.to_string(), nick, url)
            .await?;
    }

    Ok(Redirect::to("/me"))
}

#[derive(Template)]
#[template(path = "employee.html")]
struct EmployeeTemplate {
    employee: Employee,
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

#[tracing::instrument]
pub async fn employee(
    State(app): State<ServerImpl>,
    _user: SessionUser,
    Path(employee_id): Path<EmployeeId>,
) -> Result<Html<String>, AppError> {
    let employee = app
        .employee_dao
        .employee_by_id(employee_id)
        .await?
        .context("error loading me")?;

    let template = EmployeeTemplate { employee };

    Ok(Html(template.render()?))
}

#[derive(Template)]
#[template(path = "hello.html"/*, print = "all"*/)]
struct HelloTemplate {
    pub user: Option<SessionUser>,
    pub google_auth_url: Option<String>,
    pub employees: Option<Vec<Employee>>,
}

#[tracing::instrument]
pub async fn hello_world(
    State(app): State<ServerImpl>,
    user: Option<SessionUser>,
) -> Result<Html<String>, AppError> {
    let scope = "openid profile email";

    let mut employees = None::<Vec<Employee>>;
    let mut url = None::<String>;
    if user.is_some() {
        employees = Some(app.employee_dao.employees().await?);
    } else {
        let u = Url::parse_with_params(
            "https://accounts.google.com/o/oauth2/v2/auth",
            &[
                ("scope", scope),
                ("client_id", &app.cfg.client_id),
                ("response_type", "code"),
                ("redirect_uri", &app.cfg.redirect_url),
            ],
        )?
        .to_string();
        url = Some(u);
    }

    let template = HelloTemplate {
        user,
        google_auth_url: url,
        employees,
    };

    Ok(Html(template.render()?))
}
