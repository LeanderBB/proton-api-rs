use go_gpa_server::Server;
use log::LevelFilter;
use proton_api_rs::http;
use proton_api_rs::http::ClientBuilder;
use std::sync::OnceLock;

type Client = http::ureq_client::UReqClient;

static LOG_CELL: OnceLock<()> = OnceLock::new();

pub fn create_session_and_server() -> (Client, Server) {
    let debug = if let Ok(v) = std::env::var("RUST_LOG") {
        if v.eq_ignore_ascii_case("debug") {
            true
        } else {
            false
        }
    } else {
        false
    };

    LOG_CELL.get_or_init(|| {
        env_logger::builder()
            .filter_module("ureq::stream", LevelFilter::Error)
            .init();
    });

    let server = Server::new().expect("failed to create test server");
    let url = server.url().expect("Failed to get server url");

    let mut client = ClientBuilder::new().base_url(&url).allow_http();
    if debug {
        client = client.debug()
    }

    let client = client.build::<Client>().expect("Failed to create client");
    (client, server)
}
