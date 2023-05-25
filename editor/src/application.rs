use crate::dispatcher::Dispatcher;
use crate::messages::prelude::*;

pub use graphene_core::uuid::*;

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	pub dispatcher: Dispatcher,
}

impl Editor {
	/// Construct a new editor instance.
	/// Remember to provide a random seed with `editor::set_uuid_seed(seed)` before any editors can be used.
	pub fn new() -> Self {
		Self { dispatcher: Dispatcher::new() }
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Vec<FrontendMessage> {
		self.dispatcher.handle_message(message);

		let mut responses = Vec::new();
		std::mem::swap(&mut responses, &mut self.dispatcher.responses);

		responses
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) {
		self.dispatcher.poll_node_graph_evaluation(responses);
	}
}

impl Default for Editor {
	fn default() -> Self {
		Self::new()
	}
}

pub fn release_series() -> String {
	format!("Release Series: {}", env!("GRAPHITE_RELEASE_SERIES"))
}

pub fn commit_info() -> String {
	format!("{}\n{}\n{}", commit_timestamp(), commit_hash(), commit_branch())
}

pub fn commit_info_localized(localized_commit_date: &str) -> String {
	format!("{}\n{}\n{}", commit_timestamp_localized(localized_commit_date), commit_hash(), commit_branch())
}

pub fn commit_timestamp() -> String {
	format!("Date: {}", env!("GRAPHITE_GIT_COMMIT_DATE"))
}

pub fn commit_timestamp_localized(localized_commit_date: &str) -> String {
	format!("Date: {}", localized_commit_date)
}

pub fn commit_hash() -> String {
	format!("Hash: {}", &env!("GRAPHITE_GIT_COMMIT_HASH")[..8])
}

pub fn commit_branch() -> String {
	format!("Branch: {}", env!("GRAPHITE_GIT_COMMIT_BRANCH"))
}
