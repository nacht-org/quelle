wasmtime::component::bindgen!({
    path: "../../wit",
    imports: {
        "quelle:extension/http": async,
    },
    exports: {
        "register-extension": async,
        "meta": async,
        "init": async,
        "fetch-novel-info": async,
        "fetch-chapter": async,
        "simple-search": async,
        "complex-search": async,
    },
    with: {
        "quelle:extension/http/client": crate::http::HostClient,
    }
});
