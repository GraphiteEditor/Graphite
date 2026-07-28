use crate::consts::BROWSER_HOST_CONFIG_FLAG;

pub(crate) mod host;
pub(crate) mod messages;
pub(crate) mod spawn;

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct HostConfig {
	pub(crate) server: String,
	pub(crate) main_pid: u32,
	pub(crate) acceleration: bool,
	#[cfg(target_os = "linux")]
	pub(crate) frame_socket_fd: Option<std::os::fd::RawFd>,
	#[cfg(target_os = "macos")]
	pub(crate) frame_service: Option<String>,
}

impl HostConfig {
	pub(crate) fn to_arg(&self) -> String {
		let json = serde_json::to_string(self).expect("HostConfig always serializes");
		format!("{BROWSER_HOST_CONFIG_FLAG}{json}")
	}

	pub(crate) fn from_args(args: &[String]) -> Option<Self> {
		let json = args.iter().find_map(|arg| arg.strip_prefix(BROWSER_HOST_CONFIG_FLAG))?;
		match serde_json::from_str(json) {
			Ok(config) => Some(config),
			Err(e) => {
				tracing::error!("Malformed host config on the command line: {e}");
				None
			}
		}
	}
}
