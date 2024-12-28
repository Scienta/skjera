pub use crate::employee::*;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SomeAccount {
    pub id: i64,
    pub employee: i64,
    pub network: String,
    pub nick: String,
    pub url: String,
}
