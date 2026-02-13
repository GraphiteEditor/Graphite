const UPDATE_CHECK_URL: &str = "https://localhost:8080/v1/update";

async fn check_for_update(info: Info) -> Status {
	let client = reqwest::Client::new();
	let response = client.post(UPDATE_CHECK_URL).json(&info).send().await;

	match response {
		Ok(response) => {
			if response.status().is_success() {
				match response.json::<Status>().await {
					Ok(result) => result,
					Err(_) => Status::Unknown,
				}
			} else {
				Status::Unknown
			}
		}
		Err(_) => Status::Unknown,
	}
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Info {
	pub commit: Commit,
	pub version: Version,
	pub system: System,
	pub source: Option<Source>,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Commit {
	pub hash: String,
	pub time: u64,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Version {
	pub major: u64,
	pub minor: u64,
	pub patch: u64,
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
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Source {
	Flathub,
	Nixpkgs,
	Steam,
	AppleAppStore,
	WindowsStore,
	Other(String),
}

struct UpdateCheckRsponse {
	status: Status,
	messages: Vec<Message>,
}

struct Message {
	title: String,
	body: String,
	action: Action,
}

pub enum Action {
	OpenUrl(String),
	PerformAutoUpdate,
	None,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Status {
	Available(Method),
	Outdated,
	Unknown,
	Latest,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Method {
	Manual,
	Auto { url: String },
	PackageManager,
}
