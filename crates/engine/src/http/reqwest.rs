use async_trait::async_trait;

use super::HttpExecutor;
use crate::bindings::quelle::extension::http;

pub struct ReqwestExecutor {
    client: reqwest::Client,
}

impl Default for ReqwestExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestExecutor {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl HttpExecutor for ReqwestExecutor {
    async fn execute(&self, request: http::Request) -> Result<http::Response, http::ResponseError> {
        let mut builder = self.client.request(request.method.into(), &request.url);

        if let Some(params) = request.params {
            for (key, value) in params {
                builder = builder.query(&[(key, value)]);
            }
        }

        if let Some(headers) = request.headers {
            for (key, value) in headers {
                builder = builder.header(key, value);
            }
        }

        if let Some(body) = request.data {
            match body {
                http::RequestBody::Form(data) => {
                    let multipart = create_multipart(data)?;
                    builder = builder.multipart(multipart);
                }
            };
        }

        let response = builder.send().await?;
        map_response(response).await
    }
}

fn create_multipart(
    data: Vec<(String, http::FormPart)>,
) -> Result<reqwest::multipart::Form, http::ResponseError> {
    let mut form = reqwest::multipart::Form::new();
    for (name, part) in data {
        match part {
            http::FormPart::Text(value) => form = form.text(name, value),
            http::FormPart::Data(data) => {
                let mut part = reqwest::multipart::Part::bytes(data.data);

                if let Some(name) = data.name {
                    part = part.file_name(name);
                }

                if let Some(content_type) = data.content_type {
                    part = part
                        .mime_str(&content_type)
                        .map_err(|e| http::ResponseError {
                            kind: http::ResponseErrorKind::BadResponse,
                            status: None,
                            response: None,
                            message: e.to_string(),
                        })?;
                }

                form = form.part(name, part);
            }
        };
    }

    Ok(form)
}

impl From<http::Method> for reqwest::Method {
    fn from(value: http::Method) -> Self {
        match value {
            http::Method::Get => reqwest::Method::GET,
            http::Method::Post => reqwest::Method::POST,
            http::Method::Put => reqwest::Method::PUT,
            http::Method::Delete => reqwest::Method::DELETE,
            http::Method::Patch => reqwest::Method::PATCH,
            http::Method::Head => reqwest::Method::HEAD,
            http::Method::Options => reqwest::Method::OPTIONS,
        }
    }
}

async fn map_response(value: reqwest::Response) -> Result<http::Response, http::ResponseError> {
    let status = value.status().as_u16();
    let headers = value
        .headers()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
        .collect::<Vec<_>>();

    let data = value
        .bytes()
        .await
        .map_err(|e| http::ResponseError {
            kind: http::ResponseErrorKind::BadResponse,
            status: Some(status),
            response: None,
            message: e.to_string(),
        })?
        .to_vec();

    Ok(http::Response {
        status,
        headers: Some(headers),
        data: Some(data),
    })
}

impl From<reqwest::Error> for http::ResponseError {
    fn from(value: reqwest::Error) -> Self {
        http::ResponseError {
            kind: http::ResponseErrorKind::BadResponse,
            status: value.status().map(|v| v.as_u16()),
            response: None,
            message: value.to_string(),
        }
    }
}
