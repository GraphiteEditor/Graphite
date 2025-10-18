#[cfg(feature = "app")]
fn main() {
	graphite_desktop::start();
}

#[cfg(feature = "helper")]
fn main() {
	graphite_desktop::start_helper();
}

#[cfg(feature = "bundle")]
mod bundle;
#[cfg(feature = "bundle")]
fn main() {
	bundle::main().unwrap();
}
