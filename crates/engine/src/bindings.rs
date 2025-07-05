wasmtime::component::bindgen!({
    path: "../../wit",
    tracing: true,
    with: {
        "quelle:extension/http/client": crate::http::HostClient,
    }
});
