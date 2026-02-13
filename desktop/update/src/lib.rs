pub struct Info {
	pub commit: Commit,
	pub system: System,
	pub package: Package,
	pub version: Version,
}

pub struct Commit {
	pub hash: String,
	pub time: u64,
}

pub struct System {
	pub os: Os,
	pub arch: Arch,
}

pub enum Os {
	Linux,
	Mac,
	Windows,
}

pub enum Arch {
	X86_64,
	Aarch64,
}

pub struct Version {
	pub major: u64,
	pub minor: u64,
	pub patch: u64,
}

pub struct Package {
	pub url: String,
}
