use quelle_api::{
    run,
    settings::{Settings, get_settings},
    state::AppState,
    utils::{
        shutdown::shutdown_signal,
        telemetry::{get_subscriber, init_subscriber},
    },
};
use quelle_domain::registry::ExtensionRegistry;
use quelle_engine::{Executor, create_engine};
use tokio::net::TcpListener;

fn main() -> eyre::Result<()> {
    let subscriber = get_subscriber("INFO".into(), std::io::stdout);
    init_subscriber(subscriber);

    let settings: Settings = get_settings("config").expect("Failed to read config");

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main(settings))?;

    Ok(())
}

async fn async_main(settings: Settings) -> eyre::Result<()> {
    let addr = (settings.server.host.as_str(), settings.server.port);
    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to bind to the tcp stream");

    let engine = create_engine(Executor::default())?;

    let registry_dir = config.get_registry_dir();
    let registry = Box::new(LocalInstallRegistry::new(&registry_dir).await?);
    let mut store_manager = StoreManager::new(registry).await?;

    let state = AppState {
        settings: settings.clone(),
        registry: ExtensionRegistry::new(&engine, &mut store_manager),
        engine,
        store_manager,
    };

    let server = run(listener, state)
        .await
        .expect("Failed to bind the server");

    let server_future = server.with_graceful_shutdown(shutdown_signal());

    server_future.await?;

    Ok(())
}
