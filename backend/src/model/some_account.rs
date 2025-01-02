use crate::model::*;
use crate::id_type;

id_type!(SomeAccountId);

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SomeAccount {
    pub id: SomeAccountId,
    pub employee: EmployeeId,
    pub network: String,
    pub nick: String,
    pub url: String,
}
