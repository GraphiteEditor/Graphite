#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use axum::routing::get;
use axum::Router;
use fern::colors::{Color, ColoredLevelConfig};
use graphite_editor::node_graph_executor::*;
use std::sync::Mutex;

static NODE_RUNTIME_IO: Mutex<Option<NodeRuntimeIO>> = const { Mutex::new(None) };

#[tokio::main]
async fn main() {
	println!("Starting server...");

	let colors = ColoredLevelConfig::new().debug(Color::Magenta).info(Color::Green).error(Color::Red);

	fern::Dispatch::new()
		.chain(std::io::stdout())
		.level(log::LevelFilter::Trace)
		.level_for("naga", log::LevelFilter::Error)
		.level_for("wgpu-hal", log::LevelFilter::Error)
		.level_for("wgpu_hal", log::LevelFilter::Error)
		.level_for("wgpu_core", log::LevelFilter::Error)
		.format(move |out, message, record| {
			out.finish(format_args!(
				"[{}]{} {} {}",
				// This will color the log level only, not the whole line. Just a touch.
				colors.color(record.level()),
				chrono::Utc::now().format("[%Y-%m-%d %H:%M:%S]"),
				message,
				record.module_path().unwrap_or("")
			))
		})
		.apply()
		.unwrap();

	std::thread::spawn(|| loop {
		futures::executor::block_on(graphite_editor::node_graph_executor::run_node_graph());
		std::thread::sleep(std::time::Duration::from_millis(16))
	});
	graphite_editor::application::set_uuid_seed(0);

	let mut runtime_lock = NODE_RUNTIME_IO.lock().unwrap();
	*runtime_lock = Some(NodeRuntimeIO::new());
	drop(runtime_lock);

	let app = Router::new().route("/", get(|| async { "Hello, World!" }));

	// Run it with hyper on localhost:3000
	tauri::async_runtime::spawn(async {
		let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
		axum::serve(listener, app).await.unwrap();
	});

	tauri::Builder::default()
		.plugin(tauri_plugin_http::init())
		.plugin(tauri_plugin_shell::init())
		.invoke_handler(tauri::generate_handler![poll_node_graph, runtime_message])
		.setup(|_app| {
			use tauri::Manager;
			_app.get_webview_window("main").unwrap().open_devtools();
			Ok(())
		})
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}
#[tauri::command]
fn poll_node_graph() -> String {
	let vec: Vec<_> = NODE_RUNTIME_IO.lock().as_mut().unwrap().as_mut().unwrap().receive().collect();
	ron::to_string(&vec).unwrap()
}

#[tauri::command]
fn runtime_message(message: String) -> Result<(), String> {
	let message = match ron::from_str(&message) {
		Ok(message) => message,
		Err(e) => {
			log::error!("Failed to deserialize message: {}\nwith error: {}", message, e);
			return Err("Failed to deserialize message".into());
		}
	};
	let response = NODE_RUNTIME_IO.lock().as_ref().unwrap().as_ref().unwrap().send(message);
	response
}
