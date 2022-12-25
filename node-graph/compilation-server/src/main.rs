use std::sync::Arc;

use gpu_compiler_bin_wrapper::CompileRequest;

use axum::{
	extract::{Json, State},
	http::StatusCode,
	routing::{get, post},
	Router,
};

struct AppState {
	compile_dir: tempfile::TempDir,
}

#[tokio::main]
async fn main() {
	let shared_state = Arc::new(AppState {
		compile_dir: tempfile::tempdir().expect("failed to create tempdir"),
	});

	// build our application with a single route
	let app = Router::new()
		.route("/", get(|| async { "Hello from compilation server!" }))
		.route("/compile", get(|| async { "Supported targets: spirv" }))
		.route("/compile/spriv", post(post_compile_spriv))
		.with_state(shared_state);

	// run it with hyper on localhost:3000
	axum::Server::bind(&"0.0.0.0:3000".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
}

async fn post_compile_spriv(State(state): State<Arc<AppState>>, Json(compile_request): Json<CompileRequest>) -> Result<Vec<u8>, StatusCode> {
	let path = std::env::var("CARGO_MANIFEST_DIR").unwrap() + "/../gpu-compiler/Cargo.toml";
	compile_request.compile(state.compile_dir.path().to_str().expect("non utf8 tempdir path"), &path).map_err(|e| {
		eprintln!("compilation failed: {}", e);
		StatusCode::INTERNAL_SERVER_ERROR
	})
}
