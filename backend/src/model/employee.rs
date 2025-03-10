use crate::id_type;
use crate::model::*;
use async_trait::async_trait;
use sqlx::types::time::Date;
use sqlx::*;

id_type!(EmployeeId);

#[derive(Debug)]
pub struct Dao {
    pool: Pool<Postgres>,
}

impl Dao {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

impl Clone for Dao {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Employee {
    pub id: EmployeeId,
    pub email: String,
    pub name: String,
    pub dob: Option<Date>,
}

#[async_trait]
pub(crate) trait EmployeeDao {
    async fn employees(&self) -> Result<Vec<Employee>, Error>;
    async fn employee_by_id(&self, id: EmployeeId) -> Result<Option<Employee>, Error>;
    async fn employee_by_email(&self, email: String) -> Result<Option<Employee>, Error>;
    async fn employee_by_name(&self, username: String) -> Result<Option<Employee>, Error>;
    async fn insert_employee(&self, email: String, name: String) -> Result<Employee, Error>;
    async fn update(&self, employee: &Employee) -> Result<Employee, Error>;
    async fn add_some_account(
        &self,
        employee: EmployeeId,
        network: SomeNetwork,
        network_instance: Option<String>,
        authenticated: bool,
        network_avatar: Option<String>,
        subject: Option<String>,
        name: Option<String>,
        nick: Option<String>,
        url: Option<String>,
        avatar: Option<String>,
    ) -> Result<SomeAccount, Error>;

    async fn some_accounts_by_employee(
        &self,
        employee_id: EmployeeId,
    ) -> Result<Vec<SomeAccount>, Error>;

    async fn some_account_for_network(
        &self,
        employee_id: EmployeeId,
        network: String,
        network_instance: Option<String>,
    ) -> Result<Option<SomeAccount>, Error>;

    async fn update_some_account(
        &self,
        id: SomeAccountId,
        authenticated: bool,
        network_avatar: Option<String>,
        subject: Option<String>,
        name: Option<String>,
        nick: Option<String>,
        url: Option<String>,
        avatar: Option<String>,
    ) -> Result<SomeAccount, Error>;

    async fn delete_some_account(
        &self,
        id: SomeAccountId,
        employee_id: EmployeeId,
    ) -> std::result::Result<u64, Error>;
}

#[async_trait]
impl EmployeeDao for Dao
{
    // pub(crate) fn new(pool: Pool<Db>) -> EmployeeDao<Db> {
    //     EmployeeDao { pool }
    // }

    #[tracing::instrument]
    async fn employees(&self) -> Result<Vec<Employee>, Error> {
        sqlx::query_as!(Employee, "SELECT * FROM skjera.employee")
            .fetch_all(&self.pool)
            .await
    }

    #[tracing::instrument]
    async fn employee_by_id(&self, id: EmployeeId) -> Result<Option<Employee>, Error> {
        sqlx::query_as!(Employee, "SELECT * FROM skjera.employee WHERE id=$1", id.0)
            .fetch_optional(&self.pool)
            .await
    }

    #[tracing::instrument]
    async fn employee_by_email(&self, email: String) -> Result<Option<Employee>, Error> {
        sqlx::query_as!(
            Employee,
            "SELECT * FROM skjera.employee WHERE email=$1",
            email
        )
        .fetch_optional(&self.pool)
        .await
    }

    #[tracing::instrument]
    async fn employee_by_name(&self, name: String) -> Result<Option<Employee>, Error> {
        sqlx::query_as!(
            Employee,
            "SELECT * FROM skjera.employee WHERE name=$1",
            name
        )
        .fetch_optional(&self.pool)
        .await
    }

    #[tracing::instrument]
    async fn insert_employee(&self, email: String, name: String) -> Result<Employee, Error> {
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
    async fn update(&self, employee: &Employee) -> Result<Employee, Error> {
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
    async fn add_some_account(
        &self,
        employee: EmployeeId,
        network: SomeNetwork,
        network_instance: Option<String>,
        authenticated: bool,
        network_avatar: Option<String>,
        subject: Option<String>,
        name: Option<String>,
        nick: Option<String>,
        url: Option<String>,
        avatar: Option<String>,
    ) -> Result<SomeAccount, Error> {
        sqlx::query_as!(
            SomeAccount,
            "INSERT INTO skjera.some_account(employee, network, network_instance, authenticated, network_avatar, subject, name, nick, url, avatar)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING *",
            employee.0,
            network.0,
            network_instance,
            authenticated,
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
    async fn some_accounts_by_employee(
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
    async fn some_account_for_network(
        &self,
        employee_id: EmployeeId,
        network: String,
        network_instance: Option<String>,
    ) -> Result<Option<SomeAccount>, Error> {
        sqlx::query_as!(
            SomeAccount,
            "SELECT * FROM skjera.some_account WHERE employee=$1 AND network=$2 AND ((network_instance IS NULL AND $3::TEXT IS NULL) OR (network_instance=$3::TEXT))",
            employee_id.0,
            network,
            network_instance,
        )
        .fetch_optional(&self.pool)
        .await
    }

    #[tracing::instrument]
    async fn update_some_account(
        &self,
        id: SomeAccountId,
        authenticated: bool,
        network_avatar: Option<String>,
        subject: Option<String>,
        name: Option<String>,
        nick: Option<String>,
        url: Option<String>,
        avatar: Option<String>,
    ) -> Result<SomeAccount, Error> {
        sqlx::query_as!(
            SomeAccount,
            "UPDATE skjera.some_account
            SET authenticated=$1,
                network_avatar=$2,
                subject=$3,
                name=$4,
                nick=$5,
                url=$6,
                avatar=$7
            WHERE id = $8
            RETURNING *;
            ",
            authenticated,
            network_avatar,
            subject,
            name,
            nick,
            url,
            avatar,
            id.0,
        )
        .fetch_one(&self.pool)
        .await
    }

    #[tracing::instrument]
    async fn delete_some_account(
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
