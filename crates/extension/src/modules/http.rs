use eyre::{Context, eyre};

use crate::{
    http::{Client, FormPart, Method, Request, RequestBody, Response, ResponseError},
    prelude::Html,
};

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
            expect_html: false,
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

    pub fn expect_html(mut self, expect: bool) -> Self {
        self.expect_html = expect;
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

    /// Set the request body to a gRPC-Web encoded protobuf message
    #[cfg(feature = "protobuf")]
    pub fn grpc<M: prost::Message>(mut self, message: &M) -> Result<Self, eyre::Report> {
        let mut buf = Vec::new();
        message
            .encode(&mut buf)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to encode protobuf message")?;

        // Prepend the 5-byte gRPC message header: 1 byte flag (0), 4 bytes length (big endian)
        let mut grpc_buf = Vec::with_capacity(5 + buf.len());
        grpc_buf.push(0); // Compressed flag: 0 (not compressed)
        grpc_buf.extend_from_slice(&(buf.len() as u32).to_be_bytes());
        grpc_buf.extend_from_slice(&buf);

        self.data = Some(RequestBody::Raw(grpc_buf));
        self = self.add_grpc_headers();

        Ok(self)
    }

    /// Add gRPC-Web headers to the request
    #[cfg(feature = "protobuf")]
    pub fn add_grpc_headers(mut self) -> Self {
        self = self.header("Content-Type", "application/grpc-web+proto");
        self = self.header("X-Grpc-Web", "1");
        self
    }

    pub fn send(&self, client: &Client) -> Result<Response, ResponseError> {
        client.request(self)
    }

    pub fn html(self, client: &Client) -> eyre::Result<Html> {
        let response = self.expect_html(true).send(client)?;
        let response_data = response.text()?;

        let Some(html) = response_data else {
            return Err(eyre::eyre!("No content in response"));
        };

        Ok(Html::new(&html))
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

    /// Return the gRPC-Web decoded protobuf message from the response data.
    #[cfg(feature = "protobuf")]
    pub fn grpc<M: prost::Message + Default>(self) -> Result<M, eyre::Report> {
        match self.data {
            Some(data) => {
                if data.len() < 5 {
                    return Err(eyre::eyre!("Response data too short for gRPC-Web"));
                }

                // Read the gRPC-Web message header
                let compressed_flag = data[0];
                let length = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;

                if compressed_flag != 0 {
                    return Err(eyre::eyre!("Compressed gRPC messages are not supported"));
                }

                if data.len() < 5 + length {
                    return Err(eyre::eyre!("gRPC Response data length mismatch"));
                }

                let message_data = &data[5..5 + length];

                let message = M::decode(message_data)
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
