mod error;
mod game_logger;
mod games;
mod group;
mod server;

pub use std::error::Error;

use clap::{load_yaml, App};
use log::info;

fn main() -> Result<(), error::ServerError> {
	game_logger::init_logger();

	// load args
	let yaml = load_yaml!("cli.yaml");
	let matches = App::from_yaml(yaml).get_matches();

	// extract values from args
	let addr = matches.value_of("address").unwrap_or("127.0.0.1");
	let port = matches.value_of("port").unwrap_or("5001");

	// start server
	info!("create game server on {:?}", addr);
	server::run(addr, port).map(|s| s.join().unwrap())
}
