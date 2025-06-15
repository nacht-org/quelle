mod http;

use std::error;

// use bindings::quelle::core::{novel, source};
// use bindings::Extension;
use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};

use crate::bindings::Extension;
use crate::bindings::quelle::extension::{novel, source};
use crate::http::Http;

mod bindings {
    wasmtime::component::bindgen!({
        path: "../wit",
        tracing: true,
        with: {
            "quelle:extension/http/client": crate::http::HostClient,
        }
    });
}

pub struct State {
    http: Http,
}

impl State {
    pub fn new() -> Self {
        Self { http: Http::new() }
    }
}

impl novel::Host for State {}

impl source::Host for State {}

fn main() -> Result<(), Box<dyn error::Error>> {
    let engine = Engine::new(Config::new().wasm_component_model(true))?;

    let mut linker = Linker::<State>::new(&engine);
    bindings::quelle::extension::source::add_to_linker(&mut linker, |state| state)?;
    bindings::quelle::extension::novel::add_to_linker(&mut linker, |state| state)?;
    bindings::quelle::extension::http::add_to_linker(&mut linker, |state| &mut state.http)?;

    let mut store = Store::new(&engine, State::new());

    let component = Component::from_file(
        &engine,
        "target/wasm32-unknown-unknown/release/extension_scribblehub.wasm",
    )?;

    let extension = Extension::instantiate(&mut store, &component, &linker)?;
    extension.call_register_extension(&mut store)?;

    println!("Extension: {:?}", extension.call_meta(&mut store)?);

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: {} <url>", args[0]);
        std::process::exit(1);
    }

    let url = &args[1];
    let novel = extension.call_fetch_novel_info(&mut store, url)?;

    println!("Novel: {:?}", novel);

    Ok(())
}
