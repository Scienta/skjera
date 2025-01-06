use crate::id_type;
use crate::model::*;
use sqlx::types::time::Date;
use sqlx::*;

id_type!(EmployeeId);

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Employee {
    pub id: EmployeeId,
    pub email: String,
    pub name: String,
    pub dob: Option<Date>,
}

#[derive(Debug, Clone)]
pub struct EmployeeDao {
    pool: Pool<Postgres>,
}

impl EmployeeDao {
    pub(crate) fn new(pool: Pool<Postgres>) -> EmployeeDao {
        EmployeeDao { pool }
    }

    #[tracing::instrument]
    pub(crate) async fn employees(&self) -> Result<Vec<Employee>, Error> {
        sqlx::query_as!(Employee, "SELECT * FROM skjera.employee")
            .fetch_all(&self.pool)
            .await
    }

    #[tracing::instrument]
    pub(crate) async fn employee_by_id(&self, id: EmployeeId) -> Result<Option<Employee>, Error> {
        sqlx::query_as!(Employee, "SELECT * FROM skjera.employee WHERE id=$1", id.0)
            .fetch_optional(&self.pool)
            .await
    }

    #[tracing::instrument]
    pub(crate) async fn employee_by_email(&self, email: String) -> Result<Option<Employee>, Error> {
        sqlx::query_as!(
            Employee,
            "SELECT * FROM skjera.employee WHERE email=$1",
            email
        )
        .fetch_optional(&self.pool)
        .await
    }

    #[tracing::instrument]
    pub(crate) async fn insert_employee(
        &self,
        email: String,
        name: String,
    ) -> Result<Employee, Error> {
        sqlx::query_as!(
            Employee,
            "INSERT INTO skjera.employee (email, name) VALUES($1, $2) RETURNING *",
            email,
            name
        )
        .fetch_one(&self.pool)
        .await
    }

    #[tracing::instrument]
    pub(crate) async fn update(&self, employee: &Employee) -> Result<Employee, Error> {
        sqlx::query_as!(
            Employee,
            "UPDATE skjera.employee SET dob=$1 WHERE id=$2
                RETURNING *",
            employee.dob,
            employee.id.0,
        )
        .fetch_one(&self.pool)
        .await
    }

    #[tracing::instrument]
    pub(crate) async fn add_some_account(
        &self,
        employee: EmployeeId,
        network: SomeNetwork,
        authenticated: bool,
        network_instance: Option<String>,
        network_avatar: Option<String>,
        subject: Option<String>,
        name: Option<String>,
        nick: Option<String>,
        url: Option<String>,
        avatar: Option<String>,
    ) -> Result<SomeAccount, Error> {
        sqlx::query_as!(
            SomeAccount,
            "INSERT INTO skjera.some_account(employee, network, authenticated, network_instance, network_avatar, subject, name, nick, url, avatar)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING *",
            employee.0,
            network.0,
            authenticated,
            network_instance,
            network_avatar,
            subject,
            name,
            nick,
            url,
            avatar,
        )
            .fetch_one(&self.pool)
            .await
    }

    #[tracing::instrument]
    pub(crate) async fn some_accounts_by_employee(
        &self,
        employee_id: EmployeeId,
    ) -> Result<Vec<SomeAccount>, Error> {
        sqlx::query_as!(
            SomeAccount,
            "SELECT * FROM skjera.some_account WHERE employee=$1",
            employee_id.0,
        )
        .fetch_all(&self.pool)
        .await
    }

    #[tracing::instrument]
    pub(crate) async fn delete_some_account(
        &self,
        id: SomeAccountId,
        employee_id: EmployeeId,
    ) -> std::result::Result<u64, Error> {
        sqlx::query!(
            "DELETE FROM skjera.some_account WHERE id=$1 AND employee=$2",
            id.0,
            employee_id.0,
        )
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected())
    }
}
