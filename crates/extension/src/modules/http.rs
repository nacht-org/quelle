use eyre::{Context, eyre};

use crate::http::{Client, FormPart, Method, Request, RequestBody, Response, ResponseError};

impl Request {
    pub fn new(method: Method, url: String) -> Self {
        Self {
            method,
            url,
            params: None,
            data: None,
            headers: None,
            wait_for_element: None,
            wait_timeout_ms: None,
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

    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if self.params.is_none() {
            self.params = Some(Vec::new());
        }
        self.params
            .as_mut()
            .unwrap()
            .push((key.into(), value.into()));
        self
    }

    pub fn params(mut self, params: Vec<(String, String)>) -> Self {
        self.params = Some(params);
        self
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if self.headers.is_none() {
            self.headers = Some(Vec::new());
        }
        self.headers
            .as_mut()
            .unwrap()
            .push((key.into(), value.into()));
        self
    }

    pub fn headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = Some(headers);
        self
    }

    /// Wait for the specified CSS selector to be present before proceeding (Chrome only)
    pub fn wait_for_element(mut self, selector: impl Into<String>) -> Self {
        self.wait_for_element = Some(selector.into());
        self
    }

    /// Set timeout in milliseconds for element waiting (Chrome only)
    pub fn wait_timeout(mut self, timeout_ms: u32) -> Self {
        self.wait_timeout_ms = Some(timeout_ms);
        self
    }

    /// Set the request body to a protobuf-encoded message
    #[cfg(feature = "protobuf")]
    pub fn protobuf<M: prost::Message>(mut self, message: &M) -> Result<Self, eyre::Report> {
        let mut buf = Vec::new();
        message
            .encode(&mut buf)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to encode protobuf message")?;

        self.data = Some(RequestBody::Raw(buf));
        Ok(self)
    }

    pub fn send(&self, client: &Client) -> Result<Response, ResponseError> {
        client.request(self)
    }
}

impl Response {
    /// Return the text content of the response, if available.
    pub fn text(self) -> Result<Option<String>, eyre::Report> {
        match self.data {
            Some(data) => {
                let text = String::from_utf8(data).map_err(|e| eyre!(e))?;
                Ok(Some(text))
            }
            None => Ok(None),
        }
    }

    /// Return the protobuf-decoded message from the response data.
    #[cfg(feature = "protobuf")]
    pub fn protobuf<M: prost::Message + Default>(self) -> Result<M, eyre::Report> {
        match self.data {
            Some(data) => {
                let message = M::decode(&data[..])
                    .map_err(|e| eyre::eyre!("Failed to decode protobuf: {}", e))?;
                Ok(message)
            }
            None => Err(eyre::eyre!("No data in response")),
        }
    }

    /// Throws an error if the response status is not successful (2xx).
    pub fn error_for_status(self) -> Result<Self, eyre::Report> {
        if self.is_success() {
            Ok(self)
        } else {
            Err(eyre!("HTTP request failed with status {}", self.status))
        }
    }

    /// Returns whether the response status indicates success (2xx).
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

pub struct RequestFormBuilder {
    params: Vec<(String, FormPart)>,
}

impl Default for RequestFormBuilder {
    fn default() -> Self {
        Self::new()
    }
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
