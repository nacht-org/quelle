#[allow(warnings)]
mod bindings;

// pub struct ReqwestClient {
//     client: reqwest::blocking::Client,
// }

// impl ReqwestClient {
//     fn new() -> Self {
//         Self {
//             client: reqwest::blocking::Client::new(),
//         }
//     }

//     fn request(
//         &self,
//         request: outgoing::Request,
//     ) -> Result<outgoing::Response, outgoing::ResponseError> {
//         let mut builder = self.client.request(request.method.into(), request.url);

//         if let Some(params) = request.params {
//             builder = builder.query(&params);
//         }

//         if let Some(body) = request.data {
//             match body {
//                 outgoing::RequestBody::Form(data) => {
//                     let multipart = create_multipart(data);
//                     builder = builder.multipart(multipart);
//                 }
//             };
//         }

//         builder.send().map(Into::into).map_err(Into::into)
//     }
// }

// fn create_multipart(data: Vec<(String, outgoing::FormPart)>) -> reqwest::blocking::multipart::Form {
//     let mut form = reqwest::blocking::multipart::Form::new();
//     for (name, part) in data {
//         match part {
//             outgoing::FormPart::Text(value) => form = form.text(name, value),
//             outgoing::FormPart::Data(data) => {
//                 let mut part = reqwest::blocking::multipart::Part::bytes(data.data);

//                 if let Some(name) = data.name {
//                     part = part.file_name(name);
//                 }

//                 if let Some(content_type) = data.content_type {
//                     part = part.mime_str(&content_type).unwrap();
//                 }

//                 form = form.part(name, part);
//             }
//         };
//     }

//     form
// }

// impl From<outgoing::Method> for reqwest::Method {
//     fn from(value: outgoing::Method) -> Self {
//         match value {
//             outgoing::Method::Get => reqwest::Method::GET,
//             outgoing::Method::Post => reqwest::Method::POST,
//             outgoing::Method::Put => reqwest::Method::PUT,
//             outgoing::Method::Delete => reqwest::Method::DELETE,
//             outgoing::Method::Patch => reqwest::Method::PATCH,
//             outgoing::Method::Head => reqwest::Method::HEAD,
//             outgoing::Method::Options => reqwest::Method::OPTIONS,
//         }
//     }
// }

// impl From<reqwest::blocking::Response> for outgoing::Response {
//     fn from(value: reqwest::blocking::Response) -> Self {
//         let status = value.status().as_u16();
//         let headers = value
//             .headers()
//             .into_iter()
//             .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
//             .collect::<Vec<_>>();

//         let data = value.bytes().unwrap().to_vec();

//         outgoing::Response {
//             status,
//             headers: Some(headers),
//             data: Some(data),
//         }
//     }
// }

// impl From<reqwest::Error> for outgoing::ResponseError {
//     fn from(value: reqwest::Error) -> Self {
//         outgoing::ResponseError {
//             kind: outgoing::ResponseErrorKind::BadResponse,
//             status: value.status().map(|v| v.as_u16()),
//             response: None,
//             message: value.to_string(),
//         }
//     }
// }
