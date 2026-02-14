const UPDATE_CHECK_URL: &str = "https://localhost:8080/v1/update";

async fn check_for_update(info: Info) -> UpdateCheckResponse {
	let client = reqwest::Client::new();
	let response = client.post(UPDATE_CHECK_URL).json(&info).send().await;

	match response {
		Ok(response) if response.status().is_success() => {
			if let Ok(result) = response.json::<UpdateCheckResponse>().await {
				return result;
			}
		}
		_ => {}
	}
	UpdateCheckResponse {
		status: Status::Unknown,
		prompts: Vec::new(),
	}
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Info {
	pub commit: Commit,
	pub version: Version,
	pub system: System,
	pub distribution: Distribution,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Commit {
	pub hash: String,
	pub time: u64,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Version {
	Stable { major: u32, minor: u32, patch: u32 },
	Dev,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct System {
	pub os: Os,
	pub arch: Arch,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Os {
	Linux,
	Mac,
	Windows,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Arch {
	X86_64,
	Aarch64,
	Wasm32,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Distribution {
	WebOfficial,
	WebUnknown,
	Flathub,
	Nixpkgs,
	Steam,
	MacAppStore,
	WindowsStore,
	Installer,
	Portable,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UpdateCheckResponse {
	status: Option<Prompt>,
	prompts: Vec<Prompt>,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Prompt {
	title: String,
	body: String,
	resolution: Option<Resolution>,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Resolution {
	Visit { url: String },
	AutoInstall { url: String, hash: String, version: Version, commit: Commit },
	PackageManager,
}
