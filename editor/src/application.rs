use crate::dispatcher::Dispatcher;
use crate::messages::prelude::*;
pub use graphene_std::uuid::*;
use std::sync::OnceLock;

pub struct Editor {
	pub dispatcher: Dispatcher,
}

impl Editor {
	pub fn new(environment: Environment, uuid_random_seed: u64) -> Self {
		ENVIRONMENT.set(environment).expect("Editor shoud only be initialized once");
		graphene_std::uuid::set_uuid_seed(uuid_random_seed);

		Self { dispatcher: Dispatcher::new() }
	}

	#[cfg(test)]
	pub(crate) fn new_local_executor() -> (Self, crate::node_graph_executor::NodeRuntime) {
		let _ = ENVIRONMENT.set(*Editor::environment());
		graphene_std::uuid::set_uuid_seed(0);

		let (runtime, executor) = crate::node_graph_executor::NodeGraphExecutor::new_with_local_runtime();
		let editor = Self {
			dispatcher: Dispatcher::with_executor(executor),
		};

		(editor, runtime)
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Vec<FrontendMessage> {
		self.dispatcher.handle_message(message, true);

		std::mem::take(&mut self.dispatcher.responses)
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) -> Result<(), String> {
		self.dispatcher.poll_node_graph_evaluation(responses)
	}
}

static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();
impl Editor {
	#[cfg(not(test))]
	pub fn environment() -> &'static Environment {
		ENVIRONMENT.get().expect("Editor environment accessed before initialization")
	}

	#[cfg(test)]
	pub fn environment() -> &'static Environment {
		&Environment {
			platform: Platform::Desktop,
			host: Host::Linux,
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct Environment {
	pub platform: Platform,
	pub host: Host,
}
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
