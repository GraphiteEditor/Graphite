#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use graphite_editor::application::Editor;
use graphite_editor::messages::prelude::*;

// use axum::body::StreamBody;
// use axum::extract::Path;
// use axum::http;
// use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use fern::colors::{Color, ColoredLevelConfig};
// use http::{Response, StatusCode};
use std::sync::Mutex;
// use std::collections::HashMap;
// use std::sync::Arc;
// use std::sync::Mutex;

static EDITOR: Mutex<Option<Editor>> = const { Mutex::new(None) };

// async fn respond_to(id: Path<String>) -> impl IntoResponse {
// 	let builder = Response::builder().header("Access-Control-Allow-Origin", "*").status(StatusCode::OK);

// 	let guard = IMAGES.lock().unwrap();
// 	let images = guard;
// 	let image = images.as_ref().unwrap().get(&id.0).unwrap();

// 	println!("image: {:#?}", image.path);
// 	let result: Result<Vec<u8>, &str> = Ok((*image.image_data).clone());
// 	let stream = futures::stream::once(async move { result });
// 	builder.body(StreamBody::new(stream)).unwrap()
// }

#[tokio::main]
async fn main() {
	println!("Starting server...");

	let colors = ColoredLevelConfig::new().debug(Color::Magenta).info(Color::Green).error(Color::Red);

	fern::Dispatch::new()
		.chain(std::io::stdout())
		.level(log::LevelFilter::Trace)
		.format(move |out, message, record| {
			out.finish(format_args!(
				"[{}]{} {}",
				// This will color the log level only, not the whole line. Just a touch.
				colors.color(record.level()),
				chrono::Utc::now().format("[%Y-%m-%d %H:%M:%S]"),
				message
			))
		})
		.apply()
		.unwrap();

	std::thread::spawn(|| {
		let set = tokio::task::LocalSet::new();

		loop {
			set.spawn_local(graphite_editor::node_graph_executor::run_node_graph());

			std::thread::sleep(std::time::Duration::from_millis(16))
		}
	});

	// *(IMAGES.lock().unwrap()) = Some(HashMap::new());
	graphite_editor::application::set_uuid_seed(0);
	let mut editor_lock = EDITOR.lock().unwrap();
	*editor_lock = Some(Editor::new());
	drop(editor_lock);
	let app = Router::new().route("/", get(|| async { "Hello, World!" }))/*.route("/image/:id", get(respond_to))*/;

	// run it with hyper on localhost:3000
	tauri::async_runtime::spawn(async {
		let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
		axum::serve(listener, app).await.unwrap();
	});

	tauri::Builder::default()
		.plugin(tauri_plugin_http::init())
		.plugin(tauri_plugin_shell::init())
		.invoke_handler(tauri::generate_handler![set_random_seed, handle_message, poll_node_graph])
		.setup(|_app| {
			use tauri::Manager;
			_app.get_webview_window("main").unwrap().open_devtools();
			Ok(())
		})
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}
#[tauri::command]
fn set_random_seed(seed: f64) {
	graphite_editor::application::set_uuid_seed(seed as u64);
}
#[tauri::command]
async fn poll_node_graph() -> String {
	// return "[]".into();

	let mut responses = VecDeque::new();
	let mut editor_lock = EDITOR.lock().unwrap();
	editor_lock.as_mut().unwrap().poll_node_graph_evaluation(&mut responses);

	for response in &responses {
		let serialized = ron::to_string(&response.clone()).unwrap();
		if let Err(error) = ron::from_str::<FrontendMessage>(&serialized) {
			log::error!("Error deserializing message: {error}");
		}
	}

	// Process any `FrontendMessage` responses resulting from the backend processing the dispatched message
	let result: Vec<_> = responses.into_iter().collect();

	ron::to_string(&result).expect("Failed to serialize FrontendMessage")
}

#[tauri::command]
fn handle_message(message: String) -> String {
	let Ok(message) = ron::from_str::<graphite_editor::messages::message::Message>(&message) else {
		panic!("Error parsing message: {message}")
	};
	let mut editor_lock = EDITOR.lock().unwrap();
	let responses = editor_lock.as_mut().unwrap().handle_message(message);

	for response in &responses {
		let serialized = ron::to_string(&response.clone()).unwrap();
		if let Err(error) = ron::from_str::<FrontendMessage>(&serialized) {
			log::error!("Error deserializing message: {error}");
		}
	}
	// println!("handling messages");

	// Process any `FrontendMessage` responses resulting from the backend processing the dispatched message
	let result: Vec<_> = responses.into_iter().collect();

	ron::to_string(&result).expect("Failed to serialize FrontendMessage")
}
