use crate::model::*;
use crate::ServerImpl;
use async_trait::async_trait;
use axum::extract::Host;
use axum::http::Method;
use axum_extra::extract::CookieJar;
use skjera_api::apis::skjera::{ListEmployeesResponse, Skjera};
use std::collections::HashMap;

#[allow(unused_variables)]
#[async_trait]
impl Skjera for ServerImpl {
    async fn list_employees(
        &self,
        method: Method,
        host: Host,
        cookies: CookieJar,
    ) -> Result<ListEmployeesResponse, String> {
        let employees = sqlx::query_as!(Employee, "SELECT id, email, name, dob FROM skjera.employee")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                eprintln!("Database query failed: {}", e);
                e.to_string()
            })?;

        let employee_ids: Vec<i64> = employees.iter().map(|x| x.id.into()).collect();

        let some_accounts = sqlx::query_as!(
            SomeAccount,
            "SELECT id, employee, network, nick, url FROM skjera.some_account WHERE employee = ANY ($1) ORDER BY id",
            &employee_ids
        )
            .fetch_all(&self.pool)
            .await;

        if let Err(e) = some_accounts {
            let msg = format!("An error occurred: {}", e);
            eprintln!("{}", &msg);
            return Err(msg);
        }

        let mut some_accounts_by_employee: HashMap<EmployeeId, Vec<SomeAccount>> = HashMap::new();
        some_accounts.unwrap_or_default().iter().for_each(|a| {
            some_accounts_by_employee
                .entry(a.employee)
                .or_insert_with(Vec::new)
                .push(a.clone())
        });

        let empty: Vec<SomeAccount> = Vec::new();
        let employees = employees
            .iter()
            .map(|e| Self::api_employee(e, some_accounts_by_employee.get(&e.id).unwrap_or(&empty)))
            .collect();

        Ok(ListEmployeesResponse::Status200_ListOfEmployees(employees))
    }

    fn api_employee(
        e: &Employee,
        some_accounts: &Vec<SomeAccount>,
    ) -> skjera_api::models::Employee {
        skjera_api::models::Employee {
            // id: e.id,
            name: e.name.clone(),
            email: e.email.clone(),
            nick: None,
            some_accounts: some_accounts
                .iter()
                .map(ServerImpl::api_some_account)
                .collect(),
        }
    }

    fn api_some_account(s: &SomeAccount) -> skjera_api::models::SomeAccount {
        skjera_api::models::SomeAccount {
            id: s.id.into(),
            network: s.network.to_string(),
            nick: s.nick.clone().unwrap_or_default(),
            url: s.url.clone().unwrap_or_default(),
        }
    }
}
