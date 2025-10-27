fn main() {
	#[cfg(target_os = "windows")]
	{
		let mut res = winres::WindowsResource::new();
		res.set_icon("../../assets/graphite-icon-color.ico");
		res.compile().expect("Failed to compile Windows resources");
	}
}
