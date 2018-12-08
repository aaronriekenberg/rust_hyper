mod config;
mod handlers;
mod logging;
mod server;
mod utils;

use log::info;

fn install_panic_hook() {
    let original_panic_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        original_panic_hook(panic_info);
        std::process::exit(1);
    }));
}

fn build_route_configuration(
    config: &config::Configuration,
) -> Result<server::RouteConfiguration, Box<std::error::Error>> {
    let mut path_to_handler = server::RouteConfigurationHandlerMap::new();

    let index_handler = handlers::index::IndexHandler::new(config)?;
    path_to_handler.insert("/".to_string(), Box::new(index_handler));

    for command_info in config.commands() {
        let api_handler = handlers::command::api::APIHandler::new(command_info.clone());
        path_to_handler.insert(command_info.api_path().clone(), Box::new(api_handler));

        let html_handler = handlers::command::html::HTMLHandler::new(command_info.clone())?;
        path_to_handler.insert(command_info.html_path().clone(), Box::new(html_handler));
    }

    for proxy_info in config.proxies() {
        let api_handler = handlers::proxy::api::APIHandler::new(proxy_info.clone())?;
        path_to_handler.insert(proxy_info.api_path().clone(), Box::new(api_handler));

        let html_handler = handlers::proxy::html::HTMLHandler::new(proxy_info.clone())?;
        path_to_handler.insert(proxy_info.html_path().clone(), Box::new(html_handler));
    }

    for static_path_info in config.static_paths() {
        let handler = handlers::static_file::StaticFileHandler::new(
            static_path_info.fs_path().clone(),
            static_path_info.content_type(),
            static_path_info.cache_control(),
        )?;
        path_to_handler.insert(static_path_info.http_path().clone(), Box::new(handler));
    }

    let not_found_handler = handlers::not_found::NotFoundHandler;

    Ok(server::RouteConfiguration::new(
        path_to_handler,
        Box::new(not_found_handler),
    ))
}

fn main() {
    install_panic_hook();

    logging::initialize_logging().expect("failed to initialize logging");

    let executable_path = std::env::args()
        .nth(0)
        .expect("missing executable command line argument");
    info!("executable_path = '{}'", executable_path);

    let config_file = std::env::args()
        .nth(1)
        .expect("config file required as command line argument");

    let config = config::read_config(config_file).expect("error reading configuration file");
    info!("config = {:#?}", config);

    let listen_addr = config
        .listen_address()
        .parse()
        .expect("invalid listen_address");

    let route_configuration =
        build_route_configuration(&config).expect("failed to build route_configuration");

    server::run_forever(listen_addr, route_configuration).expect("server::run_forever failed");
}
