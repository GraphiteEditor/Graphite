#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

fn main() {
	println!("cargo:rerun-if-changed=../wasm/pkg/package.json");
	println!("cargo:rerun-if-changed=..");
	println!("cargo:rerun-if-changed=../wasm");
	println!("cargo:rerun-if-changed=../../frontend-svelte/wasm/pkg/package.json");
	tauri_build::build()
}
