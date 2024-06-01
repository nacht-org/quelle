use std::error;

use bindings::quelle::core::{novel, source};
use bindings::Extension;
use quelle_http::Http;
use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../wit",
        tracing: true,
        with: {
            "quelle:http": quelle_http::bindings,
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
    Extension::add_to_linker(&mut linker, |state| state)?;
    quelle_http::bindings::Http::add_to_linker(&mut linker, |state| &mut state.http)?;

    let mut store = Store::new(&engine, State::new());

    let component = Component::from_file(
        &engine,
        "target/wasm32-unknown-unknown/release/extension_scribblehub.wasm",
    )?;

    let (extension, _) = Extension::instantiate(&mut store, &component, &linker)?;

    let meta = extension.quelle_extension_meta();
    println!("Extension name: {:?}", meta.call_extension_info(&mut store));

    let instance = extension.quelle_extension_instance();

    let source = instance.source();
    let source_id = source.call_constructor(&mut store)?;

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: {} <url>", args[0]);
        std::process::exit(1);
    }

    let url = &args[1];
    let novel = source.call_novel_info(&mut store, source_id, url)?;

    println!("Novel: {:?}", novel);

    Ok(())
}
