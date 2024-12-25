#![allow(unused_qualifications)]

use http::HeaderValue;
use validator::Validate;

#[cfg(feature = "server")]
use crate::header;
use crate::{models, types::*};

      
      
      





#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct Employee {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "nick")]
    #[serde(skip_serializing_if="Option::is_none")]
    pub nick: Option<String>,

    #[serde(rename = "someAccounts")]
    pub some_accounts: Vec<models::SomeAccount>,

}


impl Employee {
    #[allow(clippy::new_without_default, clippy::too_many_arguments)]
    pub fn new(name: String, some_accounts: Vec<models::SomeAccount>, ) -> Employee {
        Employee {
            name,
            nick: None,
            some_accounts,
        }
    }
}

/// Converts the Employee value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::fmt::Display for Employee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let params: Vec<Option<String>> = vec![

            Some("name".to_string()),
            Some(self.name.to_string()),


            self.nick.as_ref().map(|nick| {
                [
                    "nick".to_string(),
                    nick.to_string(),
                ].join(",")
            }),

            // Skipping someAccounts in query parameter serialization

        ];

        write!(f, "{}", params.into_iter().flatten().collect::<Vec<_>>().join(","))
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a Employee value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for Employee {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub name: Vec<String>,
            pub nick: Vec<String>,
            pub some_accounts: Vec<Vec<models::SomeAccount>>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => return std::result::Result::Err("Missing value while parsing Employee".to_string())
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    #[allow(clippy::redundant_clone)]
                    "name" => intermediate_rep.name.push(<String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    #[allow(clippy::redundant_clone)]
                    "nick" => intermediate_rep.nick.push(<String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "someAccounts" => return std::result::Result::Err("Parsing a container in this style is not supported in Employee".to_string()),
                    _ => return std::result::Result::Err("Unexpected key while parsing Employee".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(Employee {
            name: intermediate_rep.name.into_iter().next().ok_or_else(|| "name missing in Employee".to_string())?,
            nick: intermediate_rep.nick.into_iter().next(),
            some_accounts: intermediate_rep.some_accounts.into_iter().next().ok_or_else(|| "someAccounts missing in Employee".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<Employee> and HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Employee>> for HeaderValue {
    type Error = String;

    fn try_from(hdr_value: header::IntoHeaderValue<Employee>) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match HeaderValue::from_str(&hdr_value) {
             std::result::Result::Ok(value) => std::result::Result::Ok(value),
             std::result::Result::Err(e) => std::result::Result::Err(
                 format!("Invalid header value for Employee - value: {} is invalid {}",
                     hdr_value, e))
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<HeaderValue> for header::IntoHeaderValue<Employee> {
    type Error = String;

    fn try_from(hdr_value: HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
             std::result::Result::Ok(value) => {
                    match <Employee as std::str::FromStr>::from_str(value) {
                        std::result::Result::Ok(value) => std::result::Result::Ok(header::IntoHeaderValue(value)),
                        std::result::Result::Err(err) => std::result::Result::Err(
                            format!("Unable to convert header value '{}' into Employee - {}",
                                value, err))
                    }
             },
             std::result::Result::Err(e) => std::result::Result::Err(
                 format!("Unable to convert header: {:?} to string: {}",
                     hdr_value, e))
        }
    }
}







#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct SomeAccount {
    #[serde(rename = "id")]
    pub id: i64,

    #[serde(rename = "network")]
    pub network: String,

    #[serde(rename = "url")]
    pub url: String,

    #[serde(rename = "nick")]
    pub nick: String,

}


impl SomeAccount {
    #[allow(clippy::new_without_default, clippy::too_many_arguments)]
    pub fn new(id: i64, network: String, url: String, nick: String, ) -> SomeAccount {
        SomeAccount {
            id,
            network,
            url,
            nick,
        }
    }
}

/// Converts the SomeAccount value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::fmt::Display for SomeAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let params: Vec<Option<String>> = vec![

            Some("id".to_string()),
            Some(self.id.to_string()),


            Some("network".to_string()),
            Some(self.network.to_string()),


            Some("url".to_string()),
            Some(self.url.to_string()),


            Some("nick".to_string()),
            Some(self.nick.to_string()),

        ];

        write!(f, "{}", params.into_iter().flatten().collect::<Vec<_>>().join(","))
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a SomeAccount value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for SomeAccount {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub network: Vec<String>,
            pub url: Vec<String>,
            pub nick: Vec<String>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => return std::result::Result::Err("Missing value while parsing SomeAccount".to_string())
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    #[allow(clippy::redundant_clone)]
                    "id" => intermediate_rep.id.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    #[allow(clippy::redundant_clone)]
                    "network" => intermediate_rep.network.push(<String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    #[allow(clippy::redundant_clone)]
                    "url" => intermediate_rep.url.push(<String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    #[allow(clippy::redundant_clone)]
                    "nick" => intermediate_rep.nick.push(<String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    _ => return std::result::Result::Err("Unexpected key while parsing SomeAccount".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(SomeAccount {
            id: intermediate_rep.id.into_iter().next().ok_or_else(|| "id missing in SomeAccount".to_string())?,
            network: intermediate_rep.network.into_iter().next().ok_or_else(|| "network missing in SomeAccount".to_string())?,
            url: intermediate_rep.url.into_iter().next().ok_or_else(|| "url missing in SomeAccount".to_string())?,
            nick: intermediate_rep.nick.into_iter().next().ok_or_else(|| "nick missing in SomeAccount".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<SomeAccount> and HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<SomeAccount>> for HeaderValue {
    type Error = String;

    fn try_from(hdr_value: header::IntoHeaderValue<SomeAccount>) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match HeaderValue::from_str(&hdr_value) {
             std::result::Result::Ok(value) => std::result::Result::Ok(value),
             std::result::Result::Err(e) => std::result::Result::Err(
                 format!("Invalid header value for SomeAccount - value: {} is invalid {}",
                     hdr_value, e))
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<HeaderValue> for header::IntoHeaderValue<SomeAccount> {
    type Error = String;

    fn try_from(hdr_value: HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
             std::result::Result::Ok(value) => {
                    match <SomeAccount as std::str::FromStr>::from_str(value) {
                        std::result::Result::Ok(value) => std::result::Result::Ok(header::IntoHeaderValue(value)),
                        std::result::Result::Err(err) => std::result::Result::Err(
                            format!("Unable to convert header value '{}' into SomeAccount - {}",
                                value, err))
                    }
             },
             std::result::Result::Err(e) => std::result::Result::Err(
                 format!("Unable to convert header: {:?} to string: {}",
                     hdr_value, e))
        }
    }
}




/// Enumeration of values.
/// Since this enum's variants do not hold data, we can easily define them as `#[repr(C)]`
/// which helps with FFI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "conversion", derive(frunk_enum_derive::LabelledGenericEnum))]
pub enum SomeNetwork {
    #[serde(rename = "Twitter")]
    Twitter,
    #[serde(rename = "LinkedIn")]
    LinkedIn,
    #[serde(rename = "Slack")]
    Slack,
    #[serde(rename = "GitHub")]
    GitHub,
    #[serde(rename = "GitLab")]
    GitLab,
}

impl std::fmt::Display for SomeNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            SomeNetwork::Twitter => write!(f, "Twitter"),
            SomeNetwork::LinkedIn => write!(f, "LinkedIn"),
            SomeNetwork::Slack => write!(f, "Slack"),
            SomeNetwork::GitHub => write!(f, "GitHub"),
            SomeNetwork::GitLab => write!(f, "GitLab"),
        }
    }
}

impl std::str::FromStr for SomeNetwork {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "Twitter" => std::result::Result::Ok(SomeNetwork::Twitter),
            "LinkedIn" => std::result::Result::Ok(SomeNetwork::LinkedIn),
            "Slack" => std::result::Result::Ok(SomeNetwork::Slack),
            "GitHub" => std::result::Result::Ok(SomeNetwork::GitHub),
            "GitLab" => std::result::Result::Ok(SomeNetwork::GitLab),
            _ => std::result::Result::Err(format!("Value not valid: {}", s)),
        }
    }
}

