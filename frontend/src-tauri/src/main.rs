#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use graphite_editor::application::Editor;
use graphite_editor::messages::frontend::utility_types::FrontendImageData;
use graphite_editor::messages::prelude::*;

use axum::body::StreamBody;
use axum::extract::Path;
use axum::http;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use fern::colors::{Color, ColoredLevelConfig};
use http::{Response, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Manager;

#[macro_use]
extern crate log;

mod commands;
mod helpers;

static IMAGES: Mutex<Option<HashMap<String, FrontendImageData>>> = Mutex::new(None);
pub static EDITOR: Mutex<Option<Editor>> = Mutex::new(None);

async fn respond_to(id: Path<String>) -> impl IntoResponse {
	let builder = Response::builder().header("Access-Control-Allow-Origin", "*").status(StatusCode::OK);

	let guard = IMAGES.lock().unwrap();
	let images = guard;
	let image = images.as_ref().unwrap().get(&id.0).unwrap();

	println!("image: {:#?}", image.path);
	let result: Result<Vec<u8>, &str> = Ok((*image.image_data).clone());
	let stream = futures::stream::once(async move { result });
	builder.body(StreamBody::new(stream)).unwrap()
}

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

	*(IMAGES.lock().unwrap()) = Some(HashMap::new());
	graphite_editor::application::set_uuid_seed(0);
	*(EDITOR.lock().unwrap()) = Some(Editor::new());
	let app = Router::new().route("/", get(|| async { "Hello, World!" })).route("/image/:id", get(respond_to));

	// run it with hyper on localhost:3000
	tauri::async_runtime::spawn(async {
		axum::Server::bind(&"0.0.0.0:3001".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
	});
	use commands::*;

	tauri_specta::export_to_ts(
		tauri_specta::collate_types![
			set_random_seed,
			handle_message,
			init_after_frontend_ready,
			error_dialog,
			has_crashed,
			in_development_mode,
			file_save_suffix,
			graphite_document_version,
			update_layout,
			load_preferences,
			select_document,
			new_document_dialog,
			document_open,
			open_document_file,
			open_auto_saved_document,
			trigger_auto_save,
			close_document_with_confirmation,
			request_about_graphite_dialog_with_localized_commit_date,
			bounds_of_viewports,
			on_mouse_move,
			on_wheel_scroll,
			on_mouse_down,
			on_mouse_up,
			on_double_click,
			on_key_down,
			on_key_up,
			on_change_text,
			on_font_load,
			update_bounds,
			eyedropper_sample_for_color_picker,
			update_primary_color,
			update_secondary_color,
			paste_serialized_data,
			select_layer,
			deselect_all_layers,
			move_layer_in_tree,
			set_layer_name,
			translate_canvas,
			translate_canvas_by_fraction,
			set_image_blob_url,
			set_imaginate_image_data,
			set_imaginate_generating_status,
			set_imaginate_server_status,
			process_node_graph_frame,
			connect_nodes_by_link,
			shift_node,
			disconnect_nodes,
			rectangle_intersects,
			create_node,
			select_nodes,
			paste_serialized_nodes,
			double_click_node,
			move_selected_nodes,
			toggle_preview,
			paste_image,
			toggle_layer_visibility,
			toggle_layer_expansion
		],
		"../src/bindings.ts",
	)
	.unwrap();

	tauri::Builder::default()
		.invoke_handler(tauri::generate_handler![
			set_random_seed,
			handle_message,
			init_after_frontend_ready,
			error_dialog,
			has_crashed,
			in_development_mode,
			file_save_suffix,
			graphite_document_version,
			update_layout,
			load_preferences,
			select_document,
			new_document_dialog,
			document_open,
			open_document_file,
			open_auto_saved_document,
			trigger_auto_save,
			close_document_with_confirmation,
			request_about_graphite_dialog_with_localized_commit_date,
			bounds_of_viewports,
			on_mouse_move,
			on_wheel_scroll,
			on_mouse_down,
			on_mouse_up,
			on_double_click,
			on_key_down,
			on_key_up,
			on_change_text,
			on_font_load,
			update_bounds,
			eyedropper_sample_for_color_picker,
			update_primary_color,
			update_secondary_color,
			paste_serialized_data,
			select_layer,
			deselect_all_layers,
			move_layer_in_tree,
			set_layer_name,
			translate_canvas,
			translate_canvas_by_fraction,
			set_image_blob_url,
			set_imaginate_image_data,
			set_imaginate_generating_status,
			set_imaginate_server_status,
			process_node_graph_frame,
			connect_nodes_by_link,
			shift_node,
			disconnect_nodes,
			rectangle_intersects,
			create_node,
			select_nodes,
			paste_serialized_nodes,
			double_click_node,
			move_selected_nodes,
			toggle_preview,
			paste_image,
			toggle_layer_visibility,
			toggle_layer_expansion
		])
		.setup(|app| {
			app.get_window("main").unwrap().open_devtools();
			Ok(())
		})
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}
#[tauri::command]
fn set_random_seed(seed: f64) {
	let seed = seed as u64;
	graphite_editor::application::set_uuid_seed(seed);
}

#[tauri::command]
fn handle_message(message: String) -> String {
	let Ok(message) = ron::from_str::<graphite_editor::messages::message::Message>(&message) else {
		panic!("Error parsing message: {}", message)
	};
	let mut guard = EDITOR.lock().unwrap();
	let editor = (*guard).as_mut().unwrap();
	let responses = editor.handle_message(message);

	// Sends a FrontendMessage to JavaScript
	fn send_frontend_message_to_js(message: FrontendMessage) -> FrontendMessage {
		// Special case for update image data to avoid serialization times.
		if let FrontendMessage::UpdateImageData { document_id, image_data } = message {
			let mut guard = IMAGES.lock().unwrap();
			let images = (*guard).as_mut().unwrap();
			let mut stub_data = Vec::with_capacity(image_data.len());
			for image in image_data {
				let path = image.path.clone();
				let mime = image.mime.clone();
				images.insert(format!("{:?}_{}", &image.path, document_id), image);
				stub_data.push(FrontendImageData {
					path,
					mime,
					image_data: Arc::new(Vec::new()),
				});
			}
			FrontendMessage::UpdateImageData { document_id, image_data: stub_data }
		} else {
			message
		}
	}

	for response in &responses {
		let serialized = ron::to_string(&send_frontend_message_to_js(response.clone())).unwrap();
		if let Err(error) = ron::from_str::<FrontendMessage>(&serialized) {
			log::error!("Error deserializing message: {}", error);
		}
	}

	// Process any `FrontendMessage` responses resulting from the backend processing the dispatched message
	let result: Vec<_> = responses.into_iter().map(send_frontend_message_to_js).collect();

	ron::to_string(&result).expect("Failed to serialize FrontendMessage")
}
