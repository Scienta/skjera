use async_trait::async_trait;
use axum::extract::*;
use axum_extra::extract::{CookieJar, Multipart};
use bytes::Bytes;
use http::Method;
use serde::{Deserialize, Serialize};

use crate::{models, types::*};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
#[allow(clippy::large_enum_variant)]
pub enum HelloWorldResponse {
    /// Hello World
    Status200_HelloWorld
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
#[allow(clippy::large_enum_variant)]
pub enum ListEmployeesResponse {
    /// List of employees
    Status200_ListOfEmployees
    (Vec<models::Employee>)
}


/// Skjera
#[async_trait]
#[allow(clippy::ptr_arg)]
pub trait Skjera {
    /// HelloWorld - GET /
    async fn hello_world(
    &self,
    method: Method,
    host: Host,
    cookies: CookieJar,
    ) -> Result<HelloWorldResponse, String>;

    /// ListEmployees - GET /employee
    async fn list_employees(
    &self,
    method: Method,
    host: Host,
    cookies: CookieJar,
    ) -> Result<ListEmployeesResponse, String>;
}
