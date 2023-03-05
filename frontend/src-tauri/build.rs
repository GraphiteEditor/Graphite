fn main() {
	println!("cargo:rerun-if-changed=../wasm/pkg/package.json");
	tauri_build::build()
}
