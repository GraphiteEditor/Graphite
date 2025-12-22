use crate::dispatcher::Dispatcher;
use crate::messages::prelude::*;
pub use graphene_std::uuid::*;

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	pub dispatcher: Dispatcher,
}

impl Editor {
	/// Construct the editor.
	/// Remember to provide a random seed with `editor::set_uuid_seed(seed)` before any editors can be used.
	pub fn new() -> Self {
		Self { dispatcher: Dispatcher::new() }
	}

	#[cfg(test)]
	pub(crate) fn new_local_executor() -> (Self, crate::node_graph_executor::NodeRuntime) {
		let (runtime, executor) = crate::node_graph_executor::NodeGraphExecutor::new_with_local_runtime();
		let dispatcher = Dispatcher::with_executor(executor);
		(Self { dispatcher }, runtime)
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Vec<FrontendMessage> {
		self.dispatcher.handle_message(message, true);

		std::mem::take(&mut self.dispatcher.responses)
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) -> Result<(), String> {
		self.dispatcher.poll_node_graph_evaluation(responses)
	}
}

impl Default for Editor {
	fn default() -> Self {
		Self::new()
	}
}

pub const GRAPHITE_RELEASE_SERIES: &str = env!("GRAPHITE_RELEASE_SERIES");
pub const GRAPHITE_GIT_COMMIT_DATE: &str = env!("GRAPHITE_GIT_COMMIT_DATE");
pub const GRAPHITE_GIT_COMMIT_HASH: &str = env!("GRAPHITE_GIT_COMMIT_HASH");
pub const GRAPHITE_GIT_COMMIT_BRANCH: &str = env!("GRAPHITE_GIT_COMMIT_BRANCH");

pub fn commit_info_localized(localized_commit_date: &str) -> String {
	format!(
		"Release Series: {}\n\
		Branch: {}\n\
		Commit: {}\n\
		{}",
		GRAPHITE_RELEASE_SERIES,
		GRAPHITE_GIT_COMMIT_BRANCH,
		GRAPHITE_GIT_COMMIT_HASH.get(..8).unwrap_or(GRAPHITE_GIT_COMMIT_HASH),
		localized_commit_date
	)
}
