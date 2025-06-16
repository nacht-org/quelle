use eyre::eyre;

use crate::http::{Client, FormPart, Method, Request, RequestBody, Response, ResponseError};

impl Request {
    pub fn new(method: Method, url: String) -> Self {
        Self {
            method,
            url,
            params: None,
            data: None,
            headers: None,
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new(Method::Get, url.into())
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self::new(Method::Post, url.into())
    }

    pub fn body(mut self, body: RequestBody) -> Self {
        self.data = Some(body);
        self
    }

    pub fn send(&self, client: &Client) -> Result<Response, ResponseError> {
        client.request(&self)
    }
}

impl Response {
    pub fn text(self) -> Result<Option<String>, eyre::Report> {
        match self.data {
            Some(data) => {
                let text = String::from_utf8(data).map_err(|e| eyre!(e))?;
                Ok(Some(text))
            }
            None => Ok(None),
        }
    }
}

pub struct RequestFormBuilder {
    params: Vec<(String, FormPart)>,
}

impl RequestFormBuilder {
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }

    pub fn param(mut self, key: impl Into<String>, value: impl Into<FormPart>) -> Self {
        self.params.push((key.into(), value.into()));
        self
    }

    pub fn build(self) -> RequestBody {
        RequestBody::Form(self.params)
    }
}

impl From<&str> for FormPart {
    fn from(value: &str) -> Self {
        FormPart::Text(value.to_string())
    }
}

impl From<String> for FormPart {
    fn from(value: String) -> Self {
        FormPart::Text(value)
    }
}
