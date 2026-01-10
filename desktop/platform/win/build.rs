fn main() {
	#[cfg(target_os = "windows")]
	{
		let mut res = winres::WindowsResource::new();

		res.set_icon("../../../branding/app-icons/graphite.ico");

		res.set_language(0x0409); // English (US)

		// TODO: Replace with actual version
		res.set_version_info(winres::VersionInfo::FILEVERSION, {
			const MAJOR: u64 = 0;
			const MINOR: u64 = 0;
			const PATCH: u64 = 0;
			const RELEASE: u64 = 0;
			(MAJOR << 48) | (MINOR << 32) | (PATCH << 16) | RELEASE
		});
		res.set("FileVersion", "0.0.0.0");
		res.set("ProductVersion", "0.0.0.0");

		res.set("OriginalFilename", "Graphite.exe");

		res.set("FileDescription", "Graphite");
		res.set("ProductName", "Graphite");

		// TODO: Pull this year from the Git commit date
		res.set("LegalCopyright", "Copyright © 2026 Graphite Labs, LLC");
		res.set("CompanyName", "Graphite Labs, LLC");

		res.compile().expect("Failed to compile Windows resources");
	}
}
