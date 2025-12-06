fn main() {
	#[cfg(target_os = "windows")]
	{
		let mut res = winres::WindowsResource::new();
		res.set_icon("../../../branding/app-icons/graphite.ico");
		res.set("ProductName", "Graphite");
		res.compile().expect("Failed to compile Windows resources");
	}
}
