use crate::messages::debug::utility_types::MessageLoggingVerbosity;
use crate::messages::defer::DeferMessageContext;
use crate::messages::dialog::DialogMessageContext;
use crate::messages::layout::layout_message_handler::LayoutMessageContext;
use crate::messages::prelude::*;

#[derive(Debug, Default)]
pub struct Dispatcher {
	message_queues: Vec<VecDeque<Message>>,
	pub responses: Vec<FrontendMessage>,
	pub message_handlers: DispatcherMessageHandlers,
}

#[derive(Debug, Default)]
pub struct DispatcherMessageHandlers {
	animation_message_handler: AnimationMessageHandler,
	app_window_message_handler: AppWindowMessageHandler,
	broadcast_message_handler: BroadcastMessageHandler,
	debug_message_handler: DebugMessageHandler,
	defer_message_handler: DeferMessageHandler,
	dialog_message_handler: DialogMessageHandler,
	globals_message_handler: GlobalsMessageHandler,
	input_preprocessor_message_handler: InputPreprocessorMessageHandler,
	key_mapping_message_handler: KeyMappingMessageHandler,
	layout_message_handler: LayoutMessageHandler,
	pub portfolio_message_handler: PortfolioMessageHandler,
	preferences_message_handler: PreferencesMessageHandler,
	tool_message_handler: ToolMessageHandler,
}

impl DispatcherMessageHandlers {
	pub fn with_executor(executor: crate::node_graph_executor::NodeGraphExecutor) -> Self {
		Self {
			portfolio_message_handler: PortfolioMessageHandler::with_executor(executor),
			..Default::default()
		}
	}
}

/// For optimization, these are messages guaranteed to be redundant when repeated.
/// The last occurrence of the message in the message queue is sufficient to ensure correct behavior.
/// In addition, these messages do not change any state in the backend (aside from caches).
const SIDE_EFFECT_FREE_MESSAGES: &[MessageDiscriminant] = &[
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::PropertiesPanel(
		PropertiesPanelMessageDiscriminant::Refresh,
	))),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::DocumentStructureChanged)),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::Overlays(OverlaysMessageDiscriminant::Draw))),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::NodeGraph(
		NodeGraphMessageDiscriminant::RunDocumentGraph,
	))),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::SubmitActiveGraphRender),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::RenderRulers)),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::RenderScrollbars)),
	MessageDiscriminant::Frontend(FrontendMessageDiscriminant::UpdateDocumentLayerStructure),
	MessageDiscriminant::Frontend(FrontendMessageDiscriminant::TriggerFontLoad),
];
const DEBUG_MESSAGE_BLOCK_LIST: &[MessageDiscriminant] = &[
	MessageDiscriminant::Broadcast(BroadcastMessageDiscriminant::TriggerEvent(EventMessageDiscriminant::AnimationFrame)),
	MessageDiscriminant::Animation(AnimationMessageDiscriminant::IncrementFrameCounter),
];
// TODO: Find a way to combine these with the list above. We use strings for now since these are the standard variant names used by multiple messages. But having these also type-checked would be best.
const DEBUG_MESSAGE_ENDING_BLOCK_LIST: &[&str] = &["PointerMove", "PointerOutsideViewport", "Overlays", "Draw", "CurrentTime", "Time"];

impl Dispatcher {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_executor(executor: crate::node_graph_executor::NodeGraphExecutor) -> Self {
		Self {
			message_handlers: DispatcherMessageHandlers::with_executor(executor),
			..Default::default()
		}
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

	/// Add a message to a queue so that it can be executed.
	/// If `process_after_all_current` is set, all currently queued messages (including children) will be processed first.
	/// If not set, it (and its children) will be processed as soon as possible.
	pub fn schedule_execution(message_queues: &mut Vec<VecDeque<Message>>, process_after_all_current: bool, messages: impl IntoIterator<Item = Message>) {
		match message_queues.first_mut() {
			// If there are currently messages being processed and we are processing after them, add to the end of the first queue
			Some(queue) if process_after_all_current => queue.extend(messages),
			// In all other cases, make a new inner queue and add our message there
			_ => message_queues.push(VecDeque::from_iter(messages)),
		}
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T, process_after_all_current: bool) {
		let message = message.into();

		// If we are not maintaining the buffer, simply add to the current queue
		Self::schedule_execution(&mut self.message_queues, process_after_all_current, [message]);

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

			// Print the message at a verbosity level of `info`
			self.log_message(&message, &self.message_queues, self.message_handlers.debug_message_handler.message_logging_verbosity);

			// Create a new queue for the child messages
			let mut queue = VecDeque::new();

			// Process the action by forwarding it to the relevant message handler, or saving the FrontendMessage to be sent to the frontend
			match message {
				Message::Animation(message) => {
					self.message_handlers.animation_message_handler.process_message(message, &mut queue, ());
				}
				Message::AppWindow(message) => {
					self.message_handlers.app_window_message_handler.process_message(message, &mut queue, ());
				}
				Message::Broadcast(message) => self.message_handlers.broadcast_message_handler.process_message(message, &mut queue, ()),
				Message::Debug(message) => {
					self.message_handlers.debug_message_handler.process_message(message, &mut queue, ());
				}
				Message::Defer(message) => {
					let context = DeferMessageContext {
						portfolio: &self.message_handlers.portfolio_message_handler,
					};
					self.message_handlers.defer_message_handler.process_message(message, &mut queue, context);
				}
				Message::Dialog(message) => {
					let context = DialogMessageContext {
						portfolio: &self.message_handlers.portfolio_message_handler,
						preferences: &self.message_handlers.preferences_message_handler,
						viewport_bounds: &self.message_handlers.input_preprocessor_message_handler.viewport_bounds,
					};
					self.message_handlers.dialog_message_handler.process_message(message, &mut queue, context);
				}
				Message::Frontend(message) => {
					// Handle these messages immediately by returning early
					if let FrontendMessage::TriggerFontLoad { .. } = message {
						self.responses.push(message);
						self.cleanup_queues(false);

						// Return early to avoid running the code after the match block
						return;
					} else {
						// `FrontendMessage`s are saved and will be sent to the frontend after the message queue is done being processed
						self.responses.push(message);
					}
				}
				Message::Globals(message) => {
					self.message_handlers.globals_message_handler.process_message(message, &mut queue, ());
				}
				Message::InputPreprocessor(message) => {
					let keyboard_platform = GLOBAL_PLATFORM.get().copied().unwrap_or_default().as_keyboard_platform_layout();

					self.message_handlers
						.input_preprocessor_message_handler
						.process_message(message, &mut queue, InputPreprocessorMessageContext { keyboard_platform });
				}
				Message::KeyMapping(message) => {
					let input = &self.message_handlers.input_preprocessor_message_handler;
					let actions = self.collect_actions();

					self.message_handlers
						.key_mapping_message_handler
						.process_message(message, &mut queue, KeyMappingMessageContext { input, actions });
				}
				Message::Layout(message) => {
					let action_input_mapping = &|action_to_find: &MessageDiscriminant| self.message_handlers.key_mapping_message_handler.action_input_mapping(action_to_find);
					let context = LayoutMessageContext { action_input_mapping };

					self.message_handlers.layout_message_handler.process_message(message, &mut queue, context);
				}
				Message::Portfolio(message) => {
					let ipp = &self.message_handlers.input_preprocessor_message_handler;
					let preferences = &self.message_handlers.preferences_message_handler;
					let current_tool = &self.message_handlers.tool_message_handler.tool_state.tool_data.active_tool_type;
					let message_logging_verbosity = self.message_handlers.debug_message_handler.message_logging_verbosity;
					let reset_node_definitions_on_open = self.message_handlers.portfolio_message_handler.reset_node_definitions_on_open;
					let timing_information = self.message_handlers.animation_message_handler.timing_information();
					let animation = &self.message_handlers.animation_message_handler;

					self.message_handlers.portfolio_message_handler.process_message(
						message,
						&mut queue,
						PortfolioMessageContext {
							ipp,
							preferences,
							current_tool,
							message_logging_verbosity,
							reset_node_definitions_on_open,
							timing_information,
							animation,
						},
					);
				}
				Message::Preferences(message) => {
					self.message_handlers.preferences_message_handler.process_message(message, &mut queue, ());
				}
				Message::Tool(message) => {
					let Some(document_id) = self.message_handlers.portfolio_message_handler.active_document_id() else {
						warn!("Called ToolMessage without an active document.\nGot {message:?}");
						return;
					};
					let Some(document) = self.message_handlers.portfolio_message_handler.documents.get_mut(&document_id) else {
						warn!("Called ToolMessage with an invalid active document.\nGot {message:?}");
						return;
					};

					let context = ToolMessageContext {
						document_id,
						document,
						input: &self.message_handlers.input_preprocessor_message_handler,
						persistent_data: &self.message_handlers.portfolio_message_handler.persistent_data,
						node_graph: &self.message_handlers.portfolio_message_handler.executor,
						preferences: &self.message_handlers.preferences_message_handler,
					};

					self.message_handlers.tool_message_handler.process_message(message, &mut queue, context);
				}
				Message::NoOp => {}
				Message::Batched { messages } => {
					messages.into_iter().for_each(|message| self.handle_message(message, false));
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
		list.extend(self.message_handlers.animation_message_handler.actions());
		list.extend(self.message_handlers.input_preprocessor_message_handler.actions());
		list.extend(self.message_handlers.key_mapping_message_handler.actions());
		list.extend(self.message_handlers.debug_message_handler.actions());
		if let Some(document) = self.message_handlers.portfolio_message_handler.active_document() {
			if !document.graph_view_overlay_open {
				list.extend(self.message_handlers.tool_message_handler.actions());
			}
		}
		list.extend(self.message_handlers.portfolio_message_handler.actions());
		list
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) -> Result<(), String> {
		self.message_handlers.portfolio_message_handler.poll_node_graph_evaluation(responses)
	}

	/// Create the tree structure for logging the messages as a tree
	fn create_indents(queues: &[VecDeque<Message>]) -> String {
		String::from_iter(queues.iter().enumerate().skip(1).map(|(index, queue)| {
			if index == queues.len() - 1 {
				if queue.is_empty() { "└── " } else { "├── " }
			} else if queue.is_empty() {
				"   "
			} else {
				"│    "
			}
		}))
	}

	/// Logs a message that is about to be executed, either as a tree
	/// with a discriminant or the entire payload (depending on settings)
	fn log_message(&self, message: &Message, queues: &[VecDeque<Message>], message_logging_verbosity: MessageLoggingVerbosity) {
		let discriminant = MessageDiscriminant::from(message);
		let is_blocked = DEBUG_MESSAGE_BLOCK_LIST.contains(&discriminant) || DEBUG_MESSAGE_ENDING_BLOCK_LIST.iter().any(|blocked_name| discriminant.local_name().ends_with(blocked_name));

		if !is_blocked {
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
	pub use crate::test_utils::test_prelude::*;

	/// Create an editor with three layers
	/// 1. A red rectangle
	/// 2. A blue shape
	/// 3. A green ellipse
	async fn create_editor_with_three_layers() -> EditorTestUtils {
		let mut editor = EditorTestUtils::create();

		editor.new_document().await;

		editor.select_primary_color(Color::RED).await;
		editor.draw_rect(100., 200., 300., 400.).await;

		editor.select_primary_color(Color::BLUE).await;
		editor.draw_polygon(10., 1200., 1300., 400.).await;

		editor.select_primary_color(Color::GREEN).await;
		editor.draw_ellipse(104., 1200., 1300., 400.).await;

		editor
	}

	/// - create rect, shape and ellipse
	/// - copy
	/// - paste
	/// - assert that ellipse was copied
	#[tokio::test]
	async fn copy_paste_single_layer() {
		let mut editor = create_editor_with_three_layers().await;

		let layers_before_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();
		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal }).await;
		editor
			.handle_message(PortfolioMessage::PasteIntoFolder {
				clipboard: Clipboard::Internal,
				parent: LayerNodeIdentifier::ROOT_PARENT,
				insert_index: 0,
			})
			.await;

		let layers_after_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i + 1]);
		}
	}

	#[cfg_attr(miri, ignore)]
	/// - create rect, shape and ellipse
	/// - select shape
	/// - copy
	/// - paste
	/// - assert that shape was copied
	#[tokio::test]
	async fn copy_paste_single_layer_from_middle() {
		let mut editor = create_editor_with_three_layers().await;

		let layers_before_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();
		let shape_id = editor.active_document().metadata().all_layers().nth(1).unwrap();

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![shape_id.to_node()] }).await;
		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal }).await;
		editor
			.handle_message(PortfolioMessage::PasteIntoFolder {
				clipboard: Clipboard::Internal,
				parent: LayerNodeIdentifier::ROOT_PARENT,
				insert_index: 0,
			})
			.await;

		let layers_after_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i + 1]);
		}
	}

	#[cfg_attr(miri, ignore)]
	/// - create rect, shape and ellipse
	/// - select ellipse and rect
	/// - copy
	/// - delete
	/// - create another rect
	/// - paste
	/// - paste
	#[tokio::test]
	async fn copy_paste_deleted_layers() {
		let mut editor = create_editor_with_three_layers().await;
		assert_eq!(editor.active_document().metadata().all_layers().count(), 3);

		let layers_before_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();
		let rect_id = layers_before_copy[0];
		let shape_id = layers_before_copy[1];
		let ellipse_id = layers_before_copy[2];

		editor
			.handle_message(NodeGraphMessage::SelectedNodesSet {
				nodes: vec![rect_id.to_node(), ellipse_id.to_node()],
			})
			.await;
		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal }).await;
		editor.handle_message(NodeGraphMessage::DeleteSelectedNodes { delete_children: true }).await;
		editor.draw_rect(0., 800., 12., 200.).await;
		editor
			.handle_message(PortfolioMessage::PasteIntoFolder {
				clipboard: Clipboard::Internal,
				parent: LayerNodeIdentifier::ROOT_PARENT,
				insert_index: 0,
			})
			.await;
		editor
			.handle_message(PortfolioMessage::PasteIntoFolder {
				clipboard: Clipboard::Internal,
				parent: LayerNodeIdentifier::ROOT_PARENT,
				insert_index: 0,
			})
			.await;

		let layers_after_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 6);

		println!("{layers_after_copy:?} {layers_before_copy:?}");

		assert_eq!(layers_after_copy[5], shape_id);
	}

	#[tokio::test]
	/// This test will fail when you make changes to the underlying serialization format for a document.
	async fn check_if_demo_art_opens() {
		use crate::messages::layout::utility_types::widget_prelude::*;

		let print_problem_to_terminal_on_failure = |value: &String| {
			println!();
			println!("-------------------------------------------------");
			println!("Failed test due to receiving a DisplayDialogError while loading a Graphite demo file.");
			println!();
			println!("NOTE:");
			println!("Document upgrading isn't performed in tests like when opening in the actual editor.");
			println!("You may need to open and re-save a document in the editor to apply its migrations.");
			println!();
			println!("DisplayDialogError details:");
			println!();
			println!("Description:");
			println!("{value}");
			println!("-------------------------------------------------");
			println!();

			panic!()
		};

		let mut editor = EditorTestUtils::create();

		// UNCOMMENT THIS FOR RUNNING UNDER MIRI
		//
		// let files = [
		// 	include_str!("../../demo-artwork/changing-seasons.graphite"),
		// 	include_str!("../../demo-artwork/isometric-fountain.graphite"),
		// 	include_str!("../../demo-artwork/painted-dreams.graphite"),
		// 	include_str!("../../demo-artwork/procedural-string-lights.graphite"),
		// 	include_str!("../../demo-artwork/parametric-dunescape.graphite"),
		// 	include_str!("../../demo-artwork/red-dress.graphite"),
		// 	include_str!("../../demo-artwork/valley-of-spires.graphite"),
		// ];
		// for (id, document_serialized_content) in files.iter().enumerate() {
		// let document_name = format!("document {id}");

		for (document_name, _, file_name) in crate::messages::dialog::simple_dialogs::ARTWORK {
			let document_serialized_content = std::fs::read_to_string(format!("../demo-artwork/{file_name}")).unwrap();

			assert_eq!(
				document_serialized_content.lines().count(),
				1,
				"Demo artwork '{document_name}' has more than 1 line (remember to open and re-save it in Graphite)",
			);

			let responses = editor.editor.handle_message(PortfolioMessage::OpenDocumentFile {
				document_name: Some(document_name.to_string()),
				document_path: None,
				document_serialized_content,
			});

			// Check if the graph renders
			if let Err(e) = editor.eval_graph().await {
				print_problem_to_terminal_on_failure(&format!("Failed to evaluate the graph for document '{document_name}':\n{e}"));
			}

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
