use wasmtime::component::bindgen;

bindgen!({
    world: "extension",
    path: "../wit",
});

fn main() {}
