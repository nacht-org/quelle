use wasmtime::component::ResourceTable;

use crate::bindings::quelle::extension::http;

pub struct Http {
    table: ResourceTable,
}

impl Http {
    pub fn new() -> Self {
        Self {
            table: ResourceTable::new(),
        }
    }
}

impl http::Host for Http {}

impl http::HostClient for Http {
    fn new(&mut self) -> wasmtime::component::Resource<http::Client> {
        self.table.push(http::Client::new()).unwrap()
    }

    fn request(
        &mut self,
        self_: wasmtime::component::Resource<http::Client>,
        request: http::Request,
    ) -> Result<http::Response, http::ResponseError> {
        tracing::debug!("Requesting: {:?}", request);
        self.table.get_mut(&self_).unwrap().request(request)
    }

    fn drop(&mut self, rep: wasmtime::component::Resource<http::Client>) -> wasmtime::Result<()> {
        let _ = self.table.delete(rep)?;
        Ok(())
    }
}

pub struct HostClient {
    client: reqwest::blocking::Client,
}

impl HostClient {
    fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }

    fn request(&self, request: http::Request) -> Result<http::Response, http::ResponseError> {
        let mut builder = self.client.request(request.method.into(), request.url);

        if let Some(params) = request.params {
            builder = builder.query(&params);
        }

        if let Some(body) = request.data {
            match body {
                http::RequestBody::Form(data) => {
                    let multipart = create_multipart(data);
                    builder = builder.multipart(multipart);
                }
            };
        }

        builder.send().map(Into::into).map_err(Into::into)
    }
}

fn create_multipart(data: Vec<(String, http::FormPart)>) -> reqwest::blocking::multipart::Form {
    let mut form = reqwest::blocking::multipart::Form::new();
    for (name, part) in data {
        match part {
            http::FormPart::Text(value) => form = form.text(name, value),
            http::FormPart::Data(data) => {
                let mut part = reqwest::blocking::multipart::Part::bytes(data.data);

                if let Some(name) = data.name {
                    part = part.file_name(name);
                }

                if let Some(content_type) = data.content_type {
                    part = part.mime_str(&content_type).unwrap();
                }

                form = form.part(name, part);
            }
        };
    }

    form
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

impl From<reqwest::blocking::Response> for http::Response {
    fn from(value: reqwest::blocking::Response) -> Self {
        let status = value.status().as_u16();
        let headers = value
            .headers()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
            .collect::<Vec<_>>();

        let data = value.bytes().unwrap().to_vec();

        http::Response {
            status,
            headers: Some(headers),
            data: Some(data),
        }
    }
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
