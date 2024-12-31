use sqlx::*;
use sqlx::types::time::Date;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Employee {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub dob: Option<Date>
    // pub some_accounts: Vec<SomeAccount>,
}

#[derive(Debug, Clone)]
pub struct EmployeeDao {
    pool: Pool<Postgres>,
}

impl EmployeeDao {
    pub(crate) fn new(pool: Pool<Postgres>) -> EmployeeDao {
        EmployeeDao { pool }
    }

    pub(crate) async fn employees(&self) -> Result<Vec<Employee>, Error> {
        sqlx::query_as!(
            Employee,
            "SELECT id, email, name, dob FROM skjera.employee"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub(crate) async fn employee_by_id(&self, id: i64) -> Result<Option<Employee>, Error> {
        sqlx::query_as!(
            Employee,
            "SELECT id, email, name, dob FROM skjera.employee WHERE id=$1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub(crate) async fn employee_by_email(&self, email: String) -> Result<Option<Employee>, Error> {
        sqlx::query_as!(
            Employee,
            "SELECT id, email, name, dob FROM skjera.employee WHERE email=$1",
            email
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub(crate) async fn update(&self, employee: &Employee) -> Result<Employee, Error> {
        sqlx::query_as!(
            Employee,
            "UPDATE skjera.employee SET dob=$1 WHERE id=$2
                RETURNING id, email, name, dob",
            employee.dob,
            employee.id,
        )
        .fetch_one(&self.pool)
        .await
    }
}
