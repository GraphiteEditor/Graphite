fn main() {
	let profile = std::env::var("CARGO_PROFILE").or_else(|_| std::env::var("PROFILE")).unwrap();
	println!("cargo:rustc-env=CARGO_PROFILE={}", profile);
}
