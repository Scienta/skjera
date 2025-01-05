use crate::id_type;
use crate::model::*;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::string::ToString;
use once_cell::sync::Lazy;

id_type!(SomeAccountId);

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SomeAccount {
    pub id: SomeAccountId,
    pub employee: EmployeeId,
    pub network: SomeNetwork,
    /// For systems that can have multiple instances, otherwise black.
    pub network_instance: Option<String>,
    /// The account's ID on the network
    pub subject: Option<String>,
    pub name: Option<String>,
    pub nick: Option<String>,
    pub url: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct SomeNetwork(pub String);

pub(crate) static BLUESKY: Lazy<SomeNetwork> = Lazy::new(|| SomeNetwork(String::from("bluesky")));
pub(crate) static LINKED_IN: Lazy<SomeNetwork> = Lazy::new(|| SomeNetwork(String::from("linked-in")));
pub(crate) static SLACK: Lazy<SomeNetwork> = Lazy::new(|| SomeNetwork(String::from("slack")));
pub(crate) static X: Lazy<SomeNetwork> = Lazy::new(|| SomeNetwork(String::from("x")));

impl SomeNetwork {
    // pub fn is_slack(self: &Self) -> bool {
    //     self.0 == SLACK.0
    // }
}

impl FromStr for SomeNetwork {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s: String = String::from(s);
        Ok(SomeNetwork(s.to_string()))
    }
}

impl From<String> for SomeNetwork {
    fn from(value: String) -> Self {
        SomeNetwork(value)
    }
}

impl Display for SomeNetwork {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
