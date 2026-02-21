wasmtime::component::bindgen!({
    path: "../../wit",
    imports: {
        "quelle:extension/http": async,
        "quelle:extension/scraper": async,
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
        "quelle:extension/scraper/document": crate::scraper::HostDocument,
        "quelle:extension/scraper/node": crate::scraper::HostNode,
        "quelle:extension/scraper/text-node": crate::scraper::HostTextNode,
    }
});
