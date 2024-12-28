use sqlx::*;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Employee {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub dob_month: Option<i32>,
    pub dob_day: Option<i32>,
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
            "SELECT id, email, name, dob_month, dob_day FROM skjera.employee"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub(crate) async fn employee_by_id(&self, id: i64) -> Result<Option<Employee>, Error> {
        sqlx::query_as!(
            Employee,
            "SELECT id, email, name, dob_month, dob_day FROM skjera.employee WHERE id=$1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub(crate) async fn employee_by_email(&self, email: String) -> Result<Option<Employee>, Error> {
        sqlx::query_as!(
            Employee,
            "SELECT id, email, name, dob_month, dob_day FROM skjera.employee WHERE email=$1",
            email
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub(crate) async fn update(&self, employee: &Employee) -> Result<Employee, Error> {
        sqlx::query_as!(
            Employee,
            "UPDATE skjera.employee SET dob_month=$1, dob_day=$2 WHERE id=$3
                RETURNING id, email, name, dob_month, dob_day",
            employee.dob_month,
            employee.dob_day,
            employee.id,
        )
        .fetch_one(&self.pool)
        .await
    }
}
