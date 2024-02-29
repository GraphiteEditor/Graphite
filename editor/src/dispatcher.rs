use crate::consts::{DEFAULT_FONT_FAMILY, DEFAULT_FONT_STYLE};
use crate::messages::debug::utility_types::MessageLoggingVerbosity;
use crate::messages::dialog::DialogData;
use crate::messages::prelude::*;

use graphene_core::text::Font;

#[derive(Debug, Default)]
pub struct Dispatcher {
	message_queues: Vec<VecDeque<Message>>,
	pub responses: Vec<FrontendMessage>,
	pub message_handlers: DispatcherMessageHandlers,
}

#[remain::sorted]
#[derive(Debug, Default)]
pub struct DispatcherMessageHandlers {
	broadcast_message_handler: BroadcastMessageHandler,
	debug_message_handler: DebugMessageHandler,
	dialog_message_handler: DialogMessageHandler,
	globals_message_handler: GlobalsMessageHandler,
	input_preprocessor_message_handler: InputPreprocessorMessageHandler,
	key_mapping_message_handler: KeyMappingMessageHandler,
	layout_message_handler: LayoutMessageHandler,
	pub portfolio_message_handler: PortfolioMessageHandler,
	preferences_message_handler: PreferencesMessageHandler,
	tool_message_handler: ToolMessageHandler,
	workspace_message_handler: WorkspaceMessageHandler,
}

/// For optimization, these are messages guaranteed to be redundant when repeated.
/// The last occurrence of the message in the message queue is sufficient to ensure correct behavior.
/// In addition, these messages do not change any state in the backend (aside from caches).
const SIDE_EFFECT_FREE_MESSAGES: &[MessageDiscriminant] = &[
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::NodeGraph(NodeGraphMessageDiscriminant::SendGraph))),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::PropertiesPanel(
		PropertiesPanelMessageDiscriminant::Refresh,
	))),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::DocumentStructureChanged)),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::Overlays(OverlaysMessageDiscriminant::Draw))),
	MessageDiscriminant::Frontend(FrontendMessageDiscriminant::UpdateDocumentLayerStructure),
	MessageDiscriminant::Frontend(FrontendMessageDiscriminant::TriggerFontLoad),
];

impl Dispatcher {
	pub fn new() -> Self {
		Self::default()
	}

	// If the deepest queues (higher index in queues list) are now empty (after being popped from) then remove them
	fn cleanup_queues(&mut self, leave_last: bool) {
		while self.message_queues.last().filter(|queue| queue.is_empty()).is_some() {
			if leave_last && self.message_queues.len() == 1 {
				break;
			}
			self.message_queues.pop();
		}
	}

	#[remain::check]
	pub fn handle_message<T: Into<Message>>(&mut self, message: T) {
		use Message::*;

		self.message_queues.push(VecDeque::from_iter([message.into()]));

		while let Some(message) = self.message_queues.last_mut().and_then(VecDeque::pop_front) {
			// Skip processing of this message if it will be processed later (at the end of the shallowest level queue)
			if SIDE_EFFECT_FREE_MESSAGES.contains(&message.to_discriminant()) {
				let already_in_queue = self.message_queues.first().filter(|queue| queue.contains(&message)).is_some();
				if already_in_queue {
					self.log_deferred_message(&message, &self.message_queues, self.message_handlers.debug_message_handler.message_logging_verbosity);
					self.cleanup_queues(false);
					continue;
				} else if self.message_queues.len() > 1 {
					self.log_deferred_message(&message, &self.message_queues, self.message_handlers.debug_message_handler.message_logging_verbosity);
					self.cleanup_queues(true);
					self.message_queues[0].add(message);
					continue;
				}
			}

			// Print the message at a verbosity level of `log`
			self.log_message(&message, &self.message_queues, self.message_handlers.debug_message_handler.message_logging_verbosity);

			// Create a new queue for the child messages
			let mut queue = VecDeque::new();

			// Process the action by forwarding it to the relevant message handler, or saving the FrontendMessage to be sent to the frontend
			#[remain::sorted]
			match message {
				#[remain::unsorted]
				NoOp => {}
				#[remain::unsorted]
				Init => {
					// Load persistent data from the browser database
					queue.add(FrontendMessage::TriggerLoadAutoSaveDocuments);
					queue.add(FrontendMessage::TriggerLoadPreferences);

					// Display the menu bar at the top of the window
					queue.add(MenuBarMessage::SendLayout);

					// Load the default font
					let font = Font::new(DEFAULT_FONT_FAMILY.into(), DEFAULT_FONT_STYLE.into());
					queue.add(FrontendMessage::TriggerFontLoad { font, is_default: true });
				}

				Broadcast(message) => self.message_handlers.broadcast_message_handler.process_message(message, &mut queue, ()),
				Debug(message) => {
					self.message_handlers.debug_message_handler.process_message(message, &mut queue, ());
				}
				Dialog(message) => {
					let data = DialogData {
						portfolio: &self.message_handlers.portfolio_message_handler,
						preferences: &self.message_handlers.preferences_message_handler,
					};
					self.message_handlers.dialog_message_handler.process_message(message, &mut queue, data);
				}
				Frontend(message) => {
					// Handle these messages immediately by returning early
					if let FrontendMessage::TriggerFontLoad { .. } | FrontendMessage::TriggerRefreshBoundsOfViewports = message {
						self.responses.push(message);
						self.cleanup_queues(false);

						// Return early to avoid running the code after the match block
						return;
					} else {
						// `FrontendMessage`s are saved and will be sent to the frontend after the message queue is done being processed
						self.responses.push(message);
					}
				}
				Globals(message) => {
					self.message_handlers.globals_message_handler.process_message(message, &mut queue, ());
				}
				InputPreprocessor(message) => {
					let keyboard_platform = GLOBAL_PLATFORM.get().copied().unwrap_or_default().as_keyboard_platform_layout();

					self.message_handlers.input_preprocessor_message_handler.process_message(message, &mut queue, keyboard_platform);
				}
				KeyMapping(message) => {
					let actions = self.collect_actions();

					self.message_handlers
						.key_mapping_message_handler
						.process_message(message, &mut queue, (&self.message_handlers.input_preprocessor_message_handler, actions));
				}
				Layout(message) => {
					let action_input_mapping = &|action_to_find: &MessageDiscriminant| self.message_handlers.key_mapping_message_handler.action_input_mapping(action_to_find);

					self.message_handlers.layout_message_handler.process_message(message, &mut queue, action_input_mapping);
				}
				Portfolio(message) => {
					self.message_handlers.portfolio_message_handler.process_message(
						message,
						&mut queue,
						(&self.message_handlers.input_preprocessor_message_handler, &self.message_handlers.preferences_message_handler),
					);
				}
				Preferences(message) => {
					self.message_handlers.preferences_message_handler.process_message(message, &mut queue, ());
				}
				Tool(message) => {
					if let Some(document) = self.message_handlers.portfolio_message_handler.active_document() {
						self.message_handlers.tool_message_handler.process_message(
							message,
							&mut queue,
							(
								document,
								self.message_handlers.portfolio_message_handler.active_document_id().unwrap(),
								&self.message_handlers.input_preprocessor_message_handler,
								&self.message_handlers.portfolio_message_handler.persistent_data,
								&self.message_handlers.portfolio_message_handler.executor,
							),
						);
					} else {
						warn!("Called ToolMessage without an active document.\nGot {message:?}");
					}
				}
				Workspace(message) => {
					self.message_handlers.workspace_message_handler.process_message(message, &mut queue, ());
				}
			}

			// If there are child messages, append the queue to the list of queues
			if !queue.is_empty() {
				self.message_queues.push(queue);
			}

			self.cleanup_queues(false);
		}
	}

	pub fn collect_actions(&self) -> ActionList {
		// TODO: Reduce the number of heap allocations
		let mut list = Vec::new();
		list.extend(self.message_handlers.dialog_message_handler.actions());
		list.extend(self.message_handlers.input_preprocessor_message_handler.actions());
		list.extend(self.message_handlers.key_mapping_message_handler.actions());
		list.extend(self.message_handlers.debug_message_handler.actions());
		if self.message_handlers.portfolio_message_handler.active_document().is_some() {
			list.extend(self.message_handlers.tool_message_handler.actions());
		}
		list.extend(self.message_handlers.portfolio_message_handler.actions());
		list
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) {
		self.message_handlers.portfolio_message_handler.poll_node_graph_evaluation(responses);
	}

	/// Create the tree structure for logging the messages as a tree
	fn create_indents(queues: &[VecDeque<Message>]) -> String {
		String::from_iter(queues.iter().enumerate().skip(1).map(|(index, queue)| {
			if index == queues.len() - 1 {
				if queue.is_empty() {
					"└── "
				} else {
					"├── "
				}
			} else if queue.is_empty() {
				"   "
			} else {
				"│    "
			}
		}))
	}

	/// Logs a message that is about to be executed,
	/// either as a tree with a discriminant or the entire payload (depending on settings)
	fn log_message(&self, message: &Message, queues: &[VecDeque<Message>], message_logging_verbosity: MessageLoggingVerbosity) {
		if !MessageDiscriminant::from(message).local_name().ends_with("PointerMove") {
			match message_logging_verbosity {
				MessageLoggingVerbosity::Off => {}
				MessageLoggingVerbosity::Names => {
					info!("{}{:?}", Self::create_indents(queues), message.to_discriminant());
				}
				MessageLoggingVerbosity::Contents => {
					if !(matches!(message, Message::InputPreprocessor(_))) {
						info!("Message: {}{:?}", Self::create_indents(queues), message);
					}
				}
			}
		}
	}

	/// Logs into the tree that the message is in the side effect free messages and its execution will be deferred
	fn log_deferred_message(&self, message: &Message, queues: &[VecDeque<Message>], message_logging_verbosity: MessageLoggingVerbosity) {
		if let MessageLoggingVerbosity::Names = message_logging_verbosity {
			info!("{}Deferred \"{:?}\" because it's a SIDE_EFFECT_FREE_MESSAGE", Self::create_indents(queues), message.to_discriminant());
		}
	}
}

#[cfg(test)]
mod test {
	use crate::application::Editor;
	use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
	use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
	use crate::messages::prelude::*;
	use crate::messages::tool::tool_messages::tool_prelude::ToolType;
	use crate::test_utils::EditorTestUtils;

	use graph_craft::document::NodeId;
	use graphene_core::raster::color::Color;

	fn init_logger() {
		let _ = env_logger::builder().is_test(true).try_init();
	}

	/// Create an editor instance with three layers
	/// 1. A red rectangle
	/// 2. A blue shape
	/// 3. A green ellipse
	fn create_editor_with_three_layers() -> Editor {
		init_logger();
		let mut editor = Editor::create();

		editor.new_document();

		editor.select_primary_color(Color::RED);
		editor.draw_rect(100., 200., 300., 400.);

		editor.select_primary_color(Color::BLUE);
		editor.draw_polygon(10., 1200., 1300., 400.);

		editor.select_primary_color(Color::GREEN);
		editor.draw_ellipse(104., 1200., 1300., 400.);

		editor
	}

	#[test]
	/// - create rect, shape and ellipse
	/// - copy
	/// - paste
	/// - assert that ellipse was copied
	fn copy_paste_single_layer() {
		let mut editor = create_editor_with_three_layers();

		let document_before_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().clone();
		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			parent: LayerNodeIdentifier::ROOT,
			insert_index: -1,
		});
		let document_after_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().clone();

		let layers_before_copy = document_before_copy.metadata.all_layers().collect::<Vec<_>>();
		let layers_after_copy = document_after_copy.metadata.all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i + 1]);
		}
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	/// - create rect, shape and ellipse
	/// - select shape
	/// - copy
	/// - paste
	/// - assert that shape was copied
	fn copy_paste_single_layer_from_middle() {
		let mut editor = create_editor_with_three_layers();

		let document_before_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().clone();
		let shape_id = document_before_copy.metadata.all_layers().nth(1).unwrap();

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![shape_id.to_node()] });
		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			parent: LayerNodeIdentifier::ROOT,
			insert_index: -1,
		});

		let document_after_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().clone();

		let layers_before_copy = document_before_copy.metadata.all_layers().collect::<Vec<_>>();
		let layers_after_copy = document_after_copy.metadata.all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i + 1]);
		}
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn copy_paste_folder() {
		let mut editor = create_editor_with_three_layers();

		const FOLDER_ID: NodeId = NodeId(3);

		editor.handle_message(GraphOperationMessage::NewCustomLayer {
			id: FOLDER_ID,
			nodes: HashMap::new(),
			parent: LayerNodeIdentifier::ROOT,
			insert_index: -1,
		});
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![FOLDER_ID] });

		editor.drag_tool(ToolType::Line, 0., 0., 10., 10.);
		editor.drag_tool(ToolType::Freehand, 10., 20., 30., 40.);

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![FOLDER_ID] });

		let document_before_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().clone();

		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			parent: LayerNodeIdentifier::ROOT,
			insert_index: -1,
		});

		let document_after_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().clone();

		let layers_before_copy = document_before_copy.metadata.all_layers().collect::<Vec<_>>();
		let layers_after_copy = document_after_copy.metadata.all_layers().collect::<Vec<_>>();
		let [original_folder, original_freehand, original_line, original_ellipse, original_polygon, original_rect] = layers_before_copy[..] else {
			panic!("Layers before incorrect");
		};
		let [_, _, _, folder, freehand, line, ellipse, polygon, rect] = layers_after_copy[..] else {
			panic!("Layers after incorrect");
		};
		assert_eq!(original_folder, folder);
		assert_eq!(original_freehand, freehand);
		assert_eq!(original_line, line);
		assert_eq!(original_ellipse, ellipse);
		assert_eq!(original_polygon, polygon);
		assert_eq!(original_rect, rect);
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	/// - create rect, shape and ellipse
	/// - select ellipse and rect
	/// - copy
	/// - delete
	/// - create another rect
	/// - paste
	/// - paste
	fn copy_paste_deleted_layers() {
		let mut editor = create_editor_with_three_layers();

		let document_before_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().clone();
		let mut layers = document_before_copy.metadata.all_layers();
		let rect_id = layers.next().expect("rectangle");
		let shape_id = layers.next().expect("shape");
		let ellipse_id = layers.next().expect("ellipse");

		editor.handle_message(NodeGraphMessage::SelectedNodesSet {
			nodes: vec![rect_id.to_node(), ellipse_id.to_node()],
		});
		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
		editor.handle_message(DocumentMessage::DeleteSelectedLayers);
		editor.draw_rect(0., 800., 12., 200.);
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			parent: LayerNodeIdentifier::ROOT,
			insert_index: -1,
		});
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			parent: LayerNodeIdentifier::ROOT,
			insert_index: -1,
		});

		let document_after_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().clone();

		let layers_before_copy = document_before_copy.metadata.all_layers().collect::<Vec<_>>();
		let layers_after_copy = document_after_copy.metadata.all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 6);

		println!("{:?} {:?}", layers_after_copy, layers_before_copy);

		assert_eq!(layers_after_copy[5], shape_id);
	}

	#[test]
	/// This test will fail when you make changes to the underlying serialization format for a document.
	fn check_if_demo_art_opens() {
		use crate::messages::layout::utility_types::widget_prelude::*;

		let print_problem_to_terminal_on_failure = |value: &String| {
			println!();
			println!("-------------------------------------------------");
			println!("Failed test due to receiving a DisplayDialogError while loading a Graphite demo file.");
			println!();
			println!("DisplayDialogError details:");
			println!();
			println!("Description: {value}");
			println!("-------------------------------------------------");
			println!();

			panic!()
		};

		init_logger();
		let mut editor = Editor::create();

		for (document_name, _, file_name) in crate::messages::dialog::simple_dialogs::ARTWORK {
			let document_serialized_content = std::fs::read_to_string(format!("../demo-artwork/{file_name}")).unwrap();

			assert_eq!(
				document_serialized_content.lines().count(),
				1,
				"Demo artwork '{document_name}' has more than 1 line (remember to open and re-save it in Graphite)",
			);

			let responses = editor.handle_message(PortfolioMessage::OpenDocumentFile {
				document_name: document_name.into(),
				document_serialized_content,
			});

			for response in responses {
				// Check for the existence of the file format incompatibility warning dialog after opening the test file
				if let FrontendMessage::UpdateDialogColumn1 { layout_target: _, diff } = response {
					if let DiffUpdate::SubLayout(sub_layout) = &diff[0].new_value {
						if let LayoutGroup::Row { widgets } = &sub_layout[0] {
							if let Widget::TextLabel(TextLabel { value, .. }) = &widgets[0].widget {
								print_problem_to_terminal_on_failure(value);
							}
						}
					}
				}
			}
		}
	}
}
