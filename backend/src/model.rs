#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Employee {
    pub id: i64,
    pub name: String,
    // pub some_accounts: Vec<SomeAccount>,
}

impl Employee {
    pub fn for_test(name: &str) -> Employee {
        Employee {
            id: -1,
            name: name.to_string(),
            // some_accounts: vec![],
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SomeAccount {}
