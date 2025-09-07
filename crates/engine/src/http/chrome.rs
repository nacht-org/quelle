use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use headless_chrome::{Browser, LaunchOptions, protocol::cdp::Runtime::RemoteObjectSubtype};
use serde_json::Value;
use thiserror::Error;

use super::HttpExecutor;
use crate::bindings::quelle::extension::http;

#[derive(Error, Debug)]
pub enum HeadlessChromeError {
    #[error("failed to create new tab: {0}")]
    NewTab(String),
    #[error("failed to navigate to url: {0}")]
    Navigate(String),
    #[error("failed to get content: {0}")]
    GetContent(String),
    #[error("failed to evaluate script: {0}")]
    Evaluate(String),
    #[error("failed to serialize json: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<HeadlessChromeError> for http::ResponseError {
    fn from(value: HeadlessChromeError) -> Self {
        http::ResponseError {
            kind: http::ResponseErrorKind::BadResponse,
            status: None,
            response: None,
            message: value.to_string(),
        }
    }
}

impl From<serde_json::Error> for http::ResponseError {
    fn from(value: serde_json::Error) -> Self {
        http::ResponseError {
            kind: http::ResponseErrorKind::BadResponse,
            status: None,
            response: None,
            message: value.to_string(),
        }
    }
}

pub struct HeadlessChromeExecutor {
    browser: Browser,
}

impl HeadlessChromeExecutor {
    #[tracing::instrument]
    pub fn new() -> Self {
        tracing::info!("creating new headless chrome browser instance");
        let browser = Browser::new(LaunchOptions::default_builder().build().unwrap()).unwrap();
        Self { browser }
    }
}

#[async_trait]
impl HttpExecutor for HeadlessChromeExecutor {
    #[tracing::instrument(skip_all)]
    async fn execute(&self, request: http::Request) -> Result<http::Response, http::ResponseError> {
        tracing::info!(
            url = request.url,
            "executing http request in headless chrome"
        );

        let tab = self
            .browser
            .new_tab()
            .map_err(|e| HeadlessChromeError::NewTab(e.to_string()))?;

        tab.enable_stealth_mode()
            .map_err(|e| HeadlessChromeError::NewTab(e.to_string()))?;

        if request.method == http::Method::Get {
            tracing::info!("handling GET request with direct navigation");
            let response = tab
                .navigate_to(&request.url)
                .map_err(|e| HeadlessChromeError::Navigate(e.to_string()))?
                .wait_for_element("body")
                .map_err(|e| HeadlessChromeError::GetContent(e.to_string()))?;

            let headers = vec![];
            let data = response
                .get_content()
                .map_err(|e| HeadlessChromeError::GetContent(e.to_string()))?;

            tracing::debug!("GET request successful, response: {}", &data);

            return Ok(http::Response {
                status: 200,
                headers: Some(headers),
                data: Some(data.into_bytes()),
            });
        }

        let url = request
            .url
            .parse::<url::Url>()
            .map_err(|e| HeadlessChromeError::Navigate(e.to_string()))?;

        match url.host_str() {
            Some(host) => {
                tracing::info!("navigating to site url to prepare for fetch");
                let site_url = format!("{}://{}", url.scheme(), host);
                tab.navigate_to(&site_url)
                    .map_err(|e| HeadlessChromeError::Navigate(e.to_string()))?
                    .wait_for_element("body")
                    .map_err(|e| HeadlessChromeError::GetContent(e.to_string()))?;
            }
            None => {
                return Err(
                    HeadlessChromeError::Navigate("Invalid URL: missing host".to_string()).into(),
                );
            }
        }

        let method = request.method.to_string();
        let headers = match request.headers {
            Some(headers) => serde_json::to_string(&headers)?,
            None => "undefined".to_string(),
        };

        let (body, script) = if let Some(body) = request.data {
            match body {
                http::RequestBody::Form(data) => {
                    let mut script = "const formData = new FormData();".to_string();
                    for (name, part) in data {
                        match part {
                            http::FormPart::Text(value) => {
                                script.push_str(&format!(
                                    "formData.append('{}', '{}');",
                                    name, value
                                ));
                            }
                            http::FormPart::Data(data) => {
                                let base64_data = general_purpose::STANDARD.encode(&data.data);
                                script.push_str(&format!(
                                    "formData.append('{}', new Blob([atob('{}')], {{ type: '{}' }}));",
                                    name,
                                    base64_data,
                                    data.content_type.unwrap_or_default()
                                ));
                            }
                        }
                    }
                    (Some("formData"), script)
                }
            }
        } else {
            (None, "".to_string())
        };

        let script = format!(
            r#"
(async () => {{
    {}
    const response = await fetch("{}", {{
        method: "{}",
        headers: {},
        body: {},
    }});

    const headers = {{}};
    for (const [key, value] of response.headers.entries()) {{
        headers[key] = value;
    }}

    return JSON.stringify({{
        status: response.status,
        headers: headers,
        data: await response.text(),
    }});
}})()
            "#,
            script,
            request.url,
            method,
            headers,
            body.unwrap_or("undefined")
        );
        tracing::info!("executing fetch script in browser");

        let result = tab
            .evaluate(script.trim(), true)
            .map_err(|e| HeadlessChromeError::Evaluate(e.to_string()))?;

        let value = match result.subtype {
            Some(RemoteObjectSubtype::Error) => {
                return Err(HeadlessChromeError::Evaluate(
                    result
                        .description
                        .unwrap_or_else(|| "Unknown error".to_string()),
                )
                .into());
            }
            _ => result
                .value
                .ok_or_else(|| HeadlessChromeError::Evaluate("no value returned".to_string()))?,
        };

        let value = match value {
            Value::String(s) => serde_json::from_str(&s)?,
            Value::Object(o) => o,
            _ => {
                return Err(
                    HeadlessChromeError::Evaluate("unexpected response type".to_string()).into(),
                );
            }
        };

        let status: u16 = value
            .get("status")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                HeadlessChromeError::Evaluate("missing status in response".to_string())
            })? as u16;

        let headers: Vec<(String, String)> = value
            .get("headers")
            .and_then(|v| v.as_object())
            .map(|m| {
                m.into_iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let data = value
            .get("data")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .as_bytes()
            .to_vec();

        tracing::info!("successfully executed fetch script and got response");

        Ok(http::Response {
            status,
            headers: Some(headers),
            data: Some(data),
        })
    }
}
