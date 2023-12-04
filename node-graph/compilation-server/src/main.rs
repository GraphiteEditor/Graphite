use std::{collections::HashMap, sync::Arc, sync::RwLock};

use gpu_compiler_bin_wrapper::CompileRequest;
use tower_http::cors::CorsLayer;

use axum::{
	extract::{Json, State},
	http::StatusCode,
	routing::{get, post},
	Router,
};

struct AppState {
	compile_dir: tempfile::TempDir,
	cache: RwLock<HashMap<CompileRequest, Result<Vec<u8>, StatusCode>>>,
}

#[tokio::main]
async fn main() {
	let shared_state = Arc::new(AppState {
		compile_dir: tempfile::tempdir().expect("failed to create tempdir"),
		cache: Default::default(),
	});

	// build our application with a single route
	let app = Router::new()
		.route("/", get(|| async { "Hello from compilation server!" }))
		.route("/compile", get(|| async { "Supported targets: spirv" }))
		.route("/compile/spirv", post(post_compile_spirv))
		.layer(CorsLayer::permissive())
		.with_state(shared_state);

	// run it with hyper on localhost:3000
	let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
	axum::serve(listener, app).await.unwrap();
}

async fn post_compile_spirv(State(state): State<Arc<AppState>>, Json(compile_request): Json<CompileRequest>) -> Result<Vec<u8>, StatusCode> {
	if let Some(result) = state.cache.read().unwrap().get(&compile_request) {
		return result.clone();
	}

	let path = std::env::var("CARGO_MANIFEST_DIR").unwrap() + "/../gpu-compiler/Cargo.toml";
	let result = compile_request.compile(state.compile_dir.path().to_str().expect("non utf8 tempdir path"), &path).map_err(|e| {
		eprintln!("compilation failed: {e}");
		StatusCode::INTERNAL_SERVER_ERROR
	})?;
	state.cache.write().unwrap().insert(compile_request, Ok(result.clone()));
	Ok(result)
}
