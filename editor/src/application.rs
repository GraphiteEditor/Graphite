use crate::dispatcher::Dispatcher;
use crate::messages::prelude::*;

pub use graphene_core::uuid::*;

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

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Vec<FrontendMessage> {
		self.dispatcher.handle_message(message);

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
		&GRAPHITE_GIT_COMMIT_HASH[..8],
		localized_commit_date
	)
}

#[cfg(test)]
mod test {
	use crate::messages::input_mapper::utility_types::input_mouse::ViewportBounds;
	use crate::messages::prelude::*;

	// TODO: Fix and reenable
	#[ignore]
	#[test]
	fn debug_ub() {
		use super::Message;

		let mut editor = super::Editor::new();
		let mut responses = Vec::new();

		let messages: Vec<Message> = vec![
			Message::Init,
			Message::Preferences(PreferencesMessage::Load {
				preferences: r#"{ "imaginate_server_hostname": "http://localhost:7860/", "imaginate_refresh_frequency": 1, "zoom_with_scroll": false }"#.to_string(),
			}),
			PortfolioMessage::OpenDocumentFileWithId {
				document_id: DocumentId(0),
				document_name: "".into(),
				document_is_auto_saved: true,
				document_is_saved: true,
				document_serialized_content: r#" [removed until test is reenabled] "#.into(),
			}
			.into(),
			InputPreprocessorMessage::BoundsOfViewports {
				bounds_of_viewports: vec![ViewportBounds::from_slice(&[0., 0., 1920., 1080.])],
			}
			.into(),
		];

		use futures::executor::block_on;
		for message in messages {
			block_on(crate::node_graph_executor::run_node_graph());
			let mut res = VecDeque::new();
			editor.poll_node_graph_evaluation(&mut res).expect("poll_node_graph_evaluation failed");

			let res = editor.handle_message(message);
			responses.push(res);
		}
		let responses = responses.pop().unwrap();
		// let trigger_message = responses[responses.len() - 2].clone();

		println!("responses: {responses:#?}");
	}
}
