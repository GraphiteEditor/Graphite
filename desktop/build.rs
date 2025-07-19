use std::fs::metadata;

fn main() {
	let frontend_dir = format!("{}/../frontend/dist", env!("CARGO_MANIFEST_DIR"));
	metadata(&frontend_dir).expect("Failed to find frontend directory. Please build the frontend first.");
	metadata(format!("{}/index.html", &frontend_dir)).expect("Failed to find index.html in frontend directory.");

	println!("cargo:rerun-if-changed=.");
	println!("cargo:rerun-if-changed=../frontend");
}
