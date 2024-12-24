#[derive(Debug, Clone)]
pub struct Employee {
    pub id: i64,
    pub name: String,
    pub nick: Option<String>,
    pub some_accounts: Vec<SomeAccount>,
}

impl Employee {
    pub fn new(name: &str, nick: &str) -> Employee {
        Employee {
            id: -1,
            name: name.to_string(),
            nick: Some(nick.to_string()),
            some_accounts: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SomeAccount {}
