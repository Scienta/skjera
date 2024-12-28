use crate::model::*;
use crate::{AppError, ServerImpl, SessionUser};
use anyhow::Context;
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use url;
use url::Url;

#[derive(Template)]
#[template(path = "me.html")]
struct MeTemplate {
    pub dob_month: Option<i32>,
    pub dob_day: Option<i32>,
}

pub async fn me(
    State(app): State<ServerImpl>,
    user: SessionUser,
) -> Result<Html<String>, AppError> {
    let me = app
        .employee_dao
        .employee_by_email(user.email)
        .await?
        .context("error loading me")?;

    let template = MeTemplate {
        dob_month: me.dob_month,
        dob_day: me.dob_day,
    };

    Ok(Html(template.render()?))
}

#[derive(Template)]
#[template(path = "employee.html")]
struct EmployeeTemplate {
    employee: Employee,
}

impl EmployeeTemplate {
    pub fn dob(&self) -> String {
        match (self.employee.dob_month, self.employee.dob_day) {
            (Some(m), Some(d)) => format!("{}-{}", m, d),
            _ => "".to_string(),
        }
    }
}

pub async fn employee(
    State(app): State<ServerImpl>,
    _user: SessionUser,
    Path(employee_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let employee = app
        .employee_dao
        .employee_by_id(employee_id)
        .await?
        .context("error loading me")?;

    let template = EmployeeTemplate {
        employee,
    };

    Ok(Html(template.render()?))
}

#[derive(Template)]
#[template(path = "hello.html"/*, print = "all"*/)]
struct HelloTemplate {
    pub name: String,
    pub google_auth_url: Option<String>,
    pub employees: Option<Vec<Employee>>,
}

pub async fn hello_world(
    State(app): State<ServerImpl>,
    user: Option<SessionUser>,
) -> Result<Html<String>, AppError> {
    let scope = "openid profile email";

    let mut name = "world".to_string();
    let mut employees = None::<Vec<Employee>>;
    let mut url = None::<String>;
    if user.is_some() {
        let user = user.unwrap();
        name = user.email;

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
        name,
        google_auth_url: url,
        employees,
    };

    Ok(Html(template.render()?))
}
