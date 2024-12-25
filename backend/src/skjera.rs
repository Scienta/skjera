use async_trait::async_trait;
use axum::extract::Host;
use axum::http::Method;
use axum_extra::extract::CookieJar;
use skjera_api::apis::skjera::{HelloWorldResponse, ListEmployeesResponse, Skjera};
use crate::ServerImpl;

#[allow(unused_variables)]
#[async_trait]
impl Skjera for ServerImpl {
    async fn hello_world(
        &self,
        method: Method,
        host: Host,
        cookies: CookieJar,
    ) -> Result<HelloWorldResponse, String> {
        Ok(HelloWorldResponse::Status200_HelloWorld)
    }

    async fn list_employees(
        &self,
        method: Method,
        host: Host,
        cookies: CookieJar,
    ) -> Result<ListEmployeesResponse, String> {
        let employees: Vec<skjera_api::models::Employee> = self
            .employees
            .iter()
            .map(Self::api_employee)
            .collect();
        Ok(ListEmployeesResponse::Status200_ListOfEmployees(employees))
    }
}
