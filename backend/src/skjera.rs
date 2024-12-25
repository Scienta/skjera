use crate::model;
use crate::ServerImpl;
use async_trait::async_trait;
use axum::extract::Host;
use axum::http::Method;
use axum_extra::extract::CookieJar;
use skjera_api::apis::skjera::{HelloWorldResponse, ListEmployeesResponse, Skjera};

#[allow(unused_variables)]
#[async_trait]
impl Skjera for ServerImpl {
    async fn hello_world(
        &self,
        method: Method,
        host: Host,
        cookies: CookieJar,
    ) -> Result<HelloWorldResponse, String> {
        Ok(HelloWorldResponse::Status200_HelloWorld(
            "Hello world!\n".to_string(),
        ))
    }

    async fn list_employees(
        &self,
        method: Method,
        host: Host,
        cookies: CookieJar,
    ) -> Result<ListEmployeesResponse, String> {
        let employees = sqlx::query_as!(
            model::Employee,
            "SELECT id, name FROM skjera.employee ORDER BY id DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            eprintln!("Database query failed: {}", e);
            e.to_string()
        });

        let employees = employees?;

        let employees = employees
            .iter()
            .map(|e| Self::api_employee(e, vec![]))
            .collect();

        Ok(ListEmployeesResponse::Status200_ListOfEmployees(employees))
    }
}
