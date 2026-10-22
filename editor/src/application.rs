use crate::dispatcher::Dispatcher;
use crate::messages::prelude::*;
pub use graphene_std::uuid::*;

pub struct Editor {
	pub dispatcher: Dispatcher,
}

impl Editor {
	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Vec<FrontendMessage> {
		self.dispatcher.handle_message(message, true);

		std::mem::take(&mut self.dispatcher.responses)
	}
}

#[derive(Clone, Copy, Debug)]
pub struct Environment {
	pub platform: Platform,
	pub host: Host,
}
#[cfg(test)]
pub(crate) const DEFAULT_ENVIRONMENT: Environment = Environment {
	platform: Platform::Desktop,
	host: Host::Linux,
};
#[derive(Clone, Copy, Debug)]
pub enum Platform {
	Desktop,
	Web,
}
#[derive(Clone, Copy, Debug)]
pub enum Host {
	Windows,
	Mac,
	Linux,
}
impl Environment {
	pub fn is_desktop(&self) -> bool {
		matches!(self.platform, Platform::Desktop)
	}
	pub fn is_web(&self) -> bool {
		matches!(self.platform, Platform::Web)
	}
	pub fn is_windows(&self) -> bool {
		matches!(self.host, Host::Windows)
	}
	pub fn is_mac(&self) -> bool {
		matches!(self.host, Host::Mac)
	}
	pub fn is_linux(&self) -> bool {
		matches!(self.host, Host::Linux)
	}
}

pub trait EnvironmentDisplay {
	fn fmt(&self, f: &mut std::fmt::Formatter, environment: Environment) -> std::fmt::Result;

	fn display(&self, environment: Environment) -> WithEnvironment<'_, Self> {
		WithEnvironment { value: self, environment }
	}
}

pub struct WithEnvironment<'a, T: ?Sized> {
	value: &'a T,
	environment: Environment,
}

impl<T: EnvironmentDisplay + ?Sized> std::fmt::Display for WithEnvironment<'_, T> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		self.value.fmt(f, self.environment)
	}
}

pub const GRAPHITE_RELEASE_SERIES: &str = env!("GRAPHITE_RELEASE_SERIES");
pub const GRAPHITE_GIT_COMMIT_BRANCH: Option<&str> = option_env!("GRAPHITE_GIT_COMMIT_BRANCH");
pub const GRAPHITE_GIT_COMMIT_HASH: &str = env!("GRAPHITE_GIT_COMMIT_HASH");
pub const GRAPHITE_GIT_COMMIT_DATE: &str = env!("GRAPHITE_GIT_COMMIT_DATE");

pub fn commit_info_localized(localized_commit_date: &str) -> String {
	let mut info = String::new();
	info.push_str(&format!("Release Series: {GRAPHITE_RELEASE_SERIES}\n"));
	if let Some(branch) = GRAPHITE_GIT_COMMIT_BRANCH {
		info.push_str(&format!("Branch: {branch}\n"));
	}
	info.push_str(&format!("Commit: {}\n", GRAPHITE_GIT_COMMIT_HASH.get(..8).unwrap_or(GRAPHITE_GIT_COMMIT_HASH)));
	info.push_str(localized_commit_date);
	info
}
