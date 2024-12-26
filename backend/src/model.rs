#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Employee {
    pub id: i64,
    pub email: String,
    pub name: String,
    // pub some_accounts: Vec<SomeAccount>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SomeAccount {
    pub id: i64,
    pub employee: i64,
    pub network: String,
    pub nick: String,
    pub url: String,
}
