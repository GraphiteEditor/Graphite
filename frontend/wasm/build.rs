use std::io;
use std::path::Path;

fn main() -> io::Result<()> {
	// Copy the demo artwork into the public folder so it is served.

	let source = Path::new(".").join("..").join("..").join("demo-artwork");
	let destination = Path::new(".").join("..").join("public").join("demo-artwork");
	std::fs::create_dir_all(destination.clone())?;

	for file in std::fs::read_dir(source)? {
		let path = file?.path();
		let name = path.file_name().unwrap();
		let destination = destination.clone().join(name);
		std::fs::copy(path, destination)?;
	}

	Ok(())
}
