use async_trait::async_trait;
use bytes::Bytes;
use flaregun::{CloudScraper, RequestOptions};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::sync::{Arc, Mutex};

use crate::bindings::quelle::extension::http;
use crate::http::HttpExecutor;

pub struct FlaregunExecutor {
    client: Arc<Mutex<CloudScraper>>,
}

impl FlaregunExecutor {
    pub fn new() -> Result<Self, flaregun::CloudscraperError> {
        Ok(Self {
            client: Arc::new(Mutex::new(CloudScraper::builder().build()?)),
        })
    }
}

impl From<flaregun::CloudscraperError> for http::ResponseError {
    fn from(value: flaregun::CloudscraperError) -> Self {
        http::ResponseError {
            kind: http::ResponseErrorKind::BadResponse,
            status: None,
            response: None,
            message: value.to_string(),
        }
    }
}

#[async_trait]
impl HttpExecutor for FlaregunExecutor {
    async fn execute(&self, request: http::Request) -> Result<http::Response, http::ResponseError> {
        let method = match request.method {
            http::Method::Get => reqwest::Method::GET,
            http::Method::Post => reqwest::Method::POST,
            http::Method::Put => reqwest::Method::PUT,
            http::Method::Delete => reqwest::Method::DELETE,
            http::Method::Patch => reqwest::Method::PATCH,
            http::Method::Head => reqwest::Method::HEAD,
            http::Method::Options => reqwest::Method::OPTIONS,
        };

        // Build the URL with query params appended.
        let url = if let Some(params) = &request.params {
            if !params.is_empty() {
                let mut parsed =
                    url::Url::parse(&request.url).map_err(|e| http::ResponseError {
                        kind: http::ResponseErrorKind::BadResponse,
                        status: None,
                        response: None,
                        message: e.to_string(),
                    })?;
                {
                    let mut pairs = parsed.query_pairs_mut();
                    for (k, v) in params {
                        pairs.append_pair(k, v);
                    }
                }
                parsed.to_string()
            } else {
                request.url.clone()
            }
        } else {
            request.url.clone()
        };

        // Build per-request headers.
        let extra_headers = if let Some(headers) = request.headers {
            let mut map = HeaderMap::new();
            for (k, v) in headers {
                if let (Ok(name), Ok(value)) = (
                    HeaderName::from_bytes(k.as_bytes()),
                    HeaderValue::from_str(&v),
                ) {
                    map.insert(name, value);
                }
            }
            Some(map)
        } else {
            None
        };

        // Map the request body to flaregun's RequestOptions fields.
        // Flaregun's `form` only supports simple key-value pairs, so binary
        // form parts are base64-encoded as a best-effort fallback.
        let (form, body_bytes) = match request.data {
            Some(http::RequestBody::Raw(data)) => (None, Some(Bytes::from(data))),
            Some(http::RequestBody::Form(parts)) => {
                let all_text = parts
                    .iter()
                    .all(|(_, part)| matches!(part, http::FormPart::Text(_)));

                if all_text {
                    let pairs = parts
                        .into_iter()
                        .map(|(name, part)| match part {
                            http::FormPart::Text(v) => (name, v),
                            _ => unreachable!(),
                        })
                        .collect();
                    (Some(pairs), None)
                } else {
                    use base64::Engine as _;
                    let mut pairs: Vec<(String, String)> = Vec::new();
                    for (name, part) in parts {
                        match part {
                            http::FormPart::Text(v) => pairs.push((name, v)),
                            http::FormPart::Data(data) => {
                                let encoded =
                                    base64::engine::general_purpose::STANDARD.encode(&data.data);
                                pairs.push((name, encoded));
                            }
                        }
                    }
                    (Some(pairs), None)
                }
            }
            None => (None, None),
        };

        let opts = RequestOptions {
            headers: extra_headers,
            form,
            body_bytes,
            timeout: None,
            follow_redirects: None,
        };

        // flaregun's CloudScraper::request produces a !Send future because it
        // holds a ThreadRng (backed by a non-Send Rc) across await points.
        //
        // We isolate it in spawn_blocking with its own single-threaded Tokio
        // runtime so the !Send future never crosses thread boundaries and the
        // two different reqwest versions (ours vs. flaregun's) never meet in
        // the same type signature.
        let client = Arc::clone(&self.client);
        let result = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| http::ResponseError {
                    kind: http::ResponseErrorKind::BadResponse,
                    status: None,
                    response: None,
                    message: e.to_string(),
                })?;

            #[allow(clippy::await_holding_lock)]
            rt.block_on(async move {
                let mut client = client.lock().unwrap();

                let response = client
                    .request(method, &url, opts)
                    .await
                    .map_err(http::ResponseError::from)?;

                let status = response.status().as_u16();

                let headers: Vec<(String, String)> = response
                    .headers()
                    .iter()
                    .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
                    .collect();

                let data = response
                    .bytes()
                    .await
                    .map_err(|e| http::ResponseError {
                        kind: http::ResponseErrorKind::BadResponse,
                        status: Some(status),
                        response: None,
                        message: e.to_string(),
                    })?
                    .to_vec();

                Ok::<_, http::ResponseError>((status, headers, data))
            })
        })
        .await
        .map_err(|e| http::ResponseError {
            kind: http::ResponseErrorKind::BadResponse,
            status: None,
            response: None,
            message: e.to_string(),
        })??;

        let (status, headers, data) = result;

        Ok(http::Response {
            status,
            headers: Some(headers),
            data: Some(data),
        })
    }
}
