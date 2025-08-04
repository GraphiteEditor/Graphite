use crate::messages::debug::utility_types::MessageLoggingVerbosity;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis, GroupFolderType};
use crate::messages::prelude::*;
use graphene_std::path_bool::BooleanOperation;

#[derive(Debug, Clone, Default, ExtractField)]
pub struct MenuBarMessageHandler {
	pub has_active_document: bool,
	pub canvas_tilted: bool,
	pub canvas_flipped: bool,
	pub rulers_visible: bool,
	pub node_graph_open: bool,
	pub has_selected_nodes: bool,
	pub has_selected_layers: bool,
	pub has_selection_history: (bool, bool),
	pub spreadsheet_view_open: bool,
	pub message_logging_verbosity: MessageLoggingVerbosity,
	pub reset_node_definitions_on_open: bool,
	pub make_path_editable_is_allowed: bool,
}

#[message_handler_data]
impl MessageHandler<MenuBarMessage, ()> for MenuBarMessageHandler {
	fn process_message(&mut self, message: MenuBarMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			MenuBarMessage::SendLayout => self.send_layout(responses, LayoutTarget::MenuBar),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(MenuBarMessageDiscriminant;)
	}
}

impl LayoutHolder for MenuBarMessageHandler {
	fn layout(&self) -> Layout {
		let no_active_document = !self.has_active_document;
		let node_graph_open = self.node_graph_open;
		let has_selected_nodes = self.has_selected_nodes;
		let has_selected_layers = self.has_selected_layers;
		let has_selection_history = self.has_selection_history;
		let message_logging_verbosity_off = self.message_logging_verbosity == MessageLoggingVerbosity::Off;
		let message_logging_verbosity_names = self.message_logging_verbosity == MessageLoggingVerbosity::Names;
		let message_logging_verbosity_contents = self.message_logging_verbosity == MessageLoggingVerbosity::Contents;
		let reset_node_definitions_on_open = self.reset_node_definitions_on_open;
		let make_path_editable_is_allowed = self.make_path_editable_is_allowed;

		let menu_bar_entries = vec![
			MenuBarEntry {
				icon: Some("GraphiteLogo".into()),
				action: MenuBarEntry::create_action(|_| FrontendMessage::TriggerVisitLink { url: "https://graphite.rs".into() }.into()),
				..Default::default()
			},
			MenuBarEntry::new_root(
				"File".into(),
				false,
				MenuBarEntryChildren(vec![
					vec![
						MenuBarEntry {
							label: "New…".into(),
							icon: Some("File".into()),
							action: MenuBarEntry::create_action(|_| DialogMessage::RequestNewDocumentDialog.into()),
							shortcut: action_keys!(DialogMessageDiscriminant::RequestNewDocumentDialog),
							children: MenuBarEntryChildren::empty(),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Open…".into(),
							icon: Some("Folder".into()),
							shortcut: action_keys!(PortfolioMessageDiscriminant::OpenDocument),
							action: MenuBarEntry::create_action(|_| PortfolioMessage::OpenDocument.into()),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Open Demo Artwork…".into(),
							icon: Some("Image".into()),
							action: MenuBarEntry::create_action(|_| DialogMessage::RequestDemoArtworkDialog.into()),
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Close".into(),
							icon: Some("Close".into()),
							shortcut: action_keys!(PortfolioMessageDiscriminant::CloseActiveDocumentWithConfirmation),
							action: MenuBarEntry::create_action(|_| PortfolioMessage::CloseActiveDocumentWithConfirmation.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Close All".into(),
							icon: Some("CloseAll".into()),
							shortcut: action_keys!(PortfolioMessageDiscriminant::CloseAllDocumentsWithConfirmation),
							action: MenuBarEntry::create_action(|_| PortfolioMessage::CloseAllDocumentsWithConfirmation.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Save".into(),
						icon: Some("Save".into()),
						shortcut: action_keys!(DocumentMessageDiscriminant::SaveDocument),
						action: MenuBarEntry::create_action(|_| DocumentMessage::SaveDocument.into()),
						disabled: no_active_document,
						..MenuBarEntry::default()
					}],
					vec![
						MenuBarEntry {
							label: "Import…".into(),
							icon: Some("FileImport".into()),
							shortcut: action_keys!(PortfolioMessageDiscriminant::Import),
							action: MenuBarEntry::create_action(|_| PortfolioMessage::Import.into()),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Export…".into(),
							icon: Some("FileExport".into()),
							shortcut: action_keys!(DialogMessageDiscriminant::RequestExportDialog),
							action: MenuBarEntry::create_action(|_| DialogMessage::RequestExportDialog.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Preferences…".into(),
						icon: Some("Settings".into()),
						shortcut: action_keys!(DialogMessageDiscriminant::RequestPreferencesDialog),
						action: MenuBarEntry::create_action(|_| DialogMessage::RequestPreferencesDialog.into()),
						..MenuBarEntry::default()
					}],
				]),
			),
			MenuBarEntry::new_root(
				"Edit".into(),
				false,
				MenuBarEntryChildren(vec![
					vec![
						MenuBarEntry {
							label: "Undo".into(),
							icon: Some("HistoryUndo".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::Undo),
							action: MenuBarEntry::create_action(|_| DocumentMessage::Undo.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Redo".into(),
							icon: Some("HistoryRedo".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::Redo),
							action: MenuBarEntry::create_action(|_| DocumentMessage::Redo.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Cut".into(),
							icon: Some("Cut".into()),
							shortcut: action_keys!(PortfolioMessageDiscriminant::Cut),
							action: MenuBarEntry::create_action(|_| PortfolioMessage::Cut { clipboard: Clipboard::Device }.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Copy".into(),
							icon: Some("Copy".into()),
							shortcut: action_keys!(PortfolioMessageDiscriminant::Copy),
							action: MenuBarEntry::create_action(|_| PortfolioMessage::Copy { clipboard: Clipboard::Device }.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Paste".into(),
							icon: Some("Paste".into()),
							shortcut: action_keys!(FrontendMessageDiscriminant::TriggerPaste),
							action: MenuBarEntry::create_action(|_| FrontendMessage::TriggerPaste.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Duplicate".into(),
							icon: Some("Copy".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::DuplicateSelectedLayers),
							action: MenuBarEntry::create_action(|_| DocumentMessage::DuplicateSelectedLayers.into()),
							disabled: no_active_document || !has_selected_nodes,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Delete".into(),
							icon: Some("Trash".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::DeleteSelectedLayers),
							action: MenuBarEntry::create_action(|_| DocumentMessage::DeleteSelectedLayers.into()),
							disabled: no_active_document || !has_selected_nodes,
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Convert to Infinite Canvas".into(),
						icon: Some("Artboard".into()),
						action: MenuBarEntry::create_action(|_| DocumentMessage::RemoveArtboards.into()),
						disabled: no_active_document,
						..MenuBarEntry::default()
					}],
				]),
			),
			MenuBarEntry::new_root(
				"Layer".into(),
				no_active_document,
				MenuBarEntryChildren(vec![
					vec![MenuBarEntry {
						label: "New".into(),
						icon: Some("NewLayer".into()),
						shortcut: action_keys!(DocumentMessageDiscriminant::CreateEmptyFolder),
						action: MenuBarEntry::create_action(|_| DocumentMessage::CreateEmptyFolder.into()),
						disabled: no_active_document,
						..MenuBarEntry::default()
					}],
					vec![
						MenuBarEntry {
							label: "Group".into(),
							icon: Some("Folder".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::GroupSelectedLayers),
							action: MenuBarEntry::create_action(|_| {
								DocumentMessage::GroupSelectedLayers {
									group_folder_type: GroupFolderType::Layer,
								}
								.into()
							}),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Ungroup".into(),
							icon: Some("FolderOpen".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::UngroupSelectedLayers),
							action: MenuBarEntry::create_action(|_| DocumentMessage::UngroupSelectedLayers.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Hide/Show".into(),
							icon: Some("EyeHide".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::ToggleSelectedVisibility),
							action: MenuBarEntry::create_action(|_| DocumentMessage::ToggleSelectedVisibility.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Lock/Unlock".into(),
							icon: Some("PadlockLocked".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::ToggleSelectedLocked),
							action: MenuBarEntry::create_action(|_| DocumentMessage::ToggleSelectedLocked.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Grab".into(),
							icon: Some("TransformationGrab".into()),
							shortcut: action_keys!(TransformLayerMessageDiscriminant::BeginGrab),
							action: MenuBarEntry::create_action(|_| TransformLayerMessage::BeginGrab.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Rotate".into(),
							icon: Some("TransformationRotate".into()),
							shortcut: action_keys!(TransformLayerMessageDiscriminant::BeginRotate),
							action: MenuBarEntry::create_action(|_| TransformLayerMessage::BeginRotate.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Scale".into(),
							icon: Some("TransformationScale".into()),
							shortcut: action_keys!(TransformLayerMessageDiscriminant::BeginScale),
							action: MenuBarEntry::create_action(|_| TransformLayerMessage::BeginScale.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Arrange".into(),
							icon: Some("StackHollow".into()),
							action: MenuBarEntry::no_action(),
							disabled: no_active_document || !has_selected_layers,
							children: MenuBarEntryChildren(vec![
								vec![
									MenuBarEntry {
										label: "Raise To Front".into(),
										icon: Some("Stack".into()),
										shortcut: action_keys!(DocumentMessageDiscriminant::SelectedLayersRaiseToFront),
										action: MenuBarEntry::create_action(|_| DocumentMessage::SelectedLayersRaiseToFront.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuBarEntry::default()
									},
									MenuBarEntry {
										label: "Raise".into(),
										icon: Some("StackRaise".into()),
										shortcut: action_keys!(DocumentMessageDiscriminant::SelectedLayersRaise),
										action: MenuBarEntry::create_action(|_| DocumentMessage::SelectedLayersRaise.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuBarEntry::default()
									},
									MenuBarEntry {
										label: "Lower".into(),
										icon: Some("StackLower".into()),
										shortcut: action_keys!(DocumentMessageDiscriminant::SelectedLayersLower),
										action: MenuBarEntry::create_action(|_| DocumentMessage::SelectedLayersLower.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuBarEntry::default()
									},
									MenuBarEntry {
										label: "Lower to Back".into(),
										icon: Some("StackBottom".into()),
										shortcut: action_keys!(DocumentMessageDiscriminant::SelectedLayersLowerToBack),
										action: MenuBarEntry::create_action(|_| DocumentMessage::SelectedLayersLowerToBack.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuBarEntry::default()
									},
								],
								vec![MenuBarEntry {
									label: "Reverse".into(),
									icon: Some("StackReverse".into()),
									action: MenuBarEntry::create_action(|_| DocumentMessage::SelectedLayersReverse.into()),
									disabled: no_active_document || !has_selected_layers,
									..MenuBarEntry::default()
								}],
							]),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Align".into(),
							icon: Some("AlignVerticalCenter".into()),
							action: MenuBarEntry::no_action(),
							disabled: no_active_document || !has_selected_layers,
							children: MenuBarEntryChildren({
								let choices = [
									[
										(AlignAxis::X, AlignAggregate::Min, "AlignLeft", "Align Left"),
										(AlignAxis::X, AlignAggregate::Center, "AlignHorizontalCenter", "Align Horizontal Center"),
										(AlignAxis::X, AlignAggregate::Max, "AlignRight", "Align Right"),
									],
									[
										(AlignAxis::Y, AlignAggregate::Min, "AlignTop", "Align Top"),
										(AlignAxis::Y, AlignAggregate::Center, "AlignVerticalCenter", "Align Vertical Center"),
										(AlignAxis::Y, AlignAggregate::Max, "AlignBottom", "Align Bottom"),
									],
								];

								choices
									.into_iter()
									.map(|group| {
										group
											.into_iter()
											.map(|(axis, aggregate, icon, name)| MenuBarEntry {
												label: name.into(),
												icon: Some(icon.into()),
												action: MenuBarEntry::create_action(move |_| DocumentMessage::AlignSelectedLayers { axis, aggregate }.into()),
												disabled: no_active_document || !has_selected_layers,
												..MenuBarEntry::default()
											})
											.collect()
									})
									.collect()
							}),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Flip".into(),
							icon: Some("FlipVertical".into()),
							action: MenuBarEntry::no_action(),
							disabled: no_active_document || !has_selected_layers,
							children: MenuBarEntryChildren(vec![{
								[(FlipAxis::X, "FlipHorizontal", "Horizontal"), (FlipAxis::Y, "FlipVertical", "Vertical")]
									.into_iter()
									.map(|(flip_axis, icon, name)| MenuBarEntry {
										label: name.into(),
										icon: Some(icon.into()),
										action: MenuBarEntry::create_action(move |_| DocumentMessage::FlipSelectedLayers { flip_axis }.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuBarEntry::default()
									})
									.collect()
							}]),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Turn".into(),
							icon: Some("TurnPositive90".into()),
							action: MenuBarEntry::no_action(),
							disabled: no_active_document || !has_selected_layers,
							children: MenuBarEntryChildren(vec![{
								[(-90., "TurnNegative90", "Turn -90°"), (90., "TurnPositive90", "Turn 90°")]
									.into_iter()
									.map(|(degrees, icon, name)| MenuBarEntry {
										label: name.into(),
										icon: Some(icon.into()),
										action: MenuBarEntry::create_action(move |_| DocumentMessage::RotateSelectedLayers { degrees }.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuBarEntry::default()
									})
									.collect()
							}]),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Boolean".into(),
							icon: Some("BooleanSubtractFront".into()),
							action: MenuBarEntry::no_action(),
							disabled: no_active_document || !has_selected_layers,
							children: MenuBarEntryChildren(vec![{
								let list = <BooleanOperation as graphene_std::choice_type::ChoiceTypeStatic>::list();
								list.iter()
									.flat_map(|i| i.iter())
									.map(move |(operation, info)| MenuBarEntry {
										label: info.label.to_string(),
										icon: info.icon.as_ref().map(|i| i.to_string()),
										action: MenuBarEntry::create_action(move |_| {
											let group_folder_type = GroupFolderType::BooleanOperation(*operation);
											DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
										}),
										disabled: no_active_document || !has_selected_layers,
										..MenuBarEntry::default()
									})
									.collect()
							}]),
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Make Path Editable".into(),
						icon: Some("NodeShape".into()),
						shortcut: None,
						action: MenuBarEntry::create_action(|_| NodeGraphMessage::AddPathNode.into()),
						disabled: !make_path_editable_is_allowed,
						..MenuBarEntry::default()
					}],
				]),
			),
			MenuBarEntry::new_root(
				"Select".into(),
				no_active_document,
				MenuBarEntryChildren(vec![
					vec![
						MenuBarEntry {
							label: "Select All".into(),
							icon: Some("SelectAll".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::SelectAllLayers),
							action: MenuBarEntry::create_action(|_| DocumentMessage::SelectAllLayers.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Deselect All".into(),
							icon: Some("DeselectAll".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::DeselectAllLayers),
							action: MenuBarEntry::create_action(|_| DocumentMessage::DeselectAllLayers.into()),
							disabled: no_active_document || !has_selected_nodes,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Select Parent".into(),
							icon: Some("SelectParent".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::SelectParentLayer),
							action: MenuBarEntry::create_action(|_| DocumentMessage::SelectParentLayer.into()),
							disabled: no_active_document || !has_selected_nodes,
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Previous Selection".into(),
							icon: Some("HistoryUndo".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::SelectionStepBack),
							action: MenuBarEntry::create_action(|_| DocumentMessage::SelectionStepBack.into()),
							disabled: !has_selection_history.0,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Next Selection".into(),
							icon: Some("HistoryRedo".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::SelectionStepForward),
							action: MenuBarEntry::create_action(|_| DocumentMessage::SelectionStepForward.into()),
							disabled: !has_selection_history.1,
							..MenuBarEntry::default()
						},
					],
				]),
			),
			MenuBarEntry::new_root(
				"View".into(),
				no_active_document,
				MenuBarEntryChildren(vec![
					vec![
						MenuBarEntry {
							label: "Tilt".into(),
							icon: Some("Tilt".into()),
							shortcut: action_keys!(NavigationMessageDiscriminant::BeginCanvasTilt),
							action: MenuBarEntry::create_action(|_| NavigationMessage::BeginCanvasTilt { was_dispatched_from_menu: true }.into()),
							disabled: no_active_document || node_graph_open,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Reset Tilt".into(),
							icon: Some("TiltReset".into()),
							shortcut: action_keys!(NavigationMessageDiscriminant::CanvasTiltSet),
							action: MenuBarEntry::create_action(|_| NavigationMessage::CanvasTiltSet { angle_radians: 0.into() }.into()),
							disabled: no_active_document || node_graph_open || !self.canvas_tilted,
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Zoom In".into(),
							icon: Some("ZoomIn".into()),
							shortcut: action_keys!(NavigationMessageDiscriminant::CanvasZoomIncrease),
							action: MenuBarEntry::create_action(|_| NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Zoom Out".into(),
							icon: Some("ZoomOut".into()),
							shortcut: action_keys!(NavigationMessageDiscriminant::CanvasZoomDecrease),
							action: MenuBarEntry::create_action(|_| NavigationMessage::CanvasZoomDecrease { center_on_mouse: false }.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Zoom to Selection".into(),
							icon: Some("FrameSelected".into()),
							shortcut: action_keys!(NavigationMessageDiscriminant::FitViewportToSelection),
							action: MenuBarEntry::create_action(|_| NavigationMessage::FitViewportToSelection.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Zoom to Fit".into(),
							icon: Some("FrameAll".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::ZoomCanvasToFitAll),
							action: MenuBarEntry::create_action(|_| DocumentMessage::ZoomCanvasToFitAll.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Zoom to 100%".into(),
							icon: Some("Zoom1x".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::ZoomCanvasTo100Percent),
							action: MenuBarEntry::create_action(|_| DocumentMessage::ZoomCanvasTo100Percent.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Zoom to 200%".into(),
							icon: Some("Zoom2x".into()),
							shortcut: action_keys!(DocumentMessageDiscriminant::ZoomCanvasTo200Percent),
							action: MenuBarEntry::create_action(|_| DocumentMessage::ZoomCanvasTo200Percent.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Flip".into(),
						icon: Some(if self.canvas_flipped { "CheckboxChecked" } else { "CheckboxUnchecked" }.into()),
						shortcut: action_keys!(NavigationMessageDiscriminant::CanvasFlip),
						action: MenuBarEntry::create_action(|_| NavigationMessage::CanvasFlip.into()),
						disabled: no_active_document || node_graph_open,
						..MenuBarEntry::default()
					}],
					vec![MenuBarEntry {
						label: "Rulers".into(),
						icon: Some(if self.rulers_visible { "CheckboxChecked" } else { "CheckboxUnchecked" }.into()),
						shortcut: action_keys!(PortfolioMessageDiscriminant::ToggleRulers),
						action: MenuBarEntry::create_action(|_| PortfolioMessage::ToggleRulers.into()),
						disabled: no_active_document,
						..MenuBarEntry::default()
					}],
					vec![MenuBarEntry {
						label: "Window: Spreadsheet".into(),
						icon: Some(if self.spreadsheet_view_open { "CheckboxChecked" } else { "CheckboxUnchecked" }.into()),
						action: MenuBarEntry::create_action(|_| SpreadsheetMessage::ToggleOpen.into()),
						disabled: no_active_document,
						..MenuBarEntry::default()
					}],
				]),
			),
			MenuBarEntry::new_root(
				"Help".into(),
				true,
				MenuBarEntryChildren(vec![
					vec![MenuBarEntry {
						label: "About Graphite…".into(),
						icon: Some("GraphiteLogo".into()),
						action: MenuBarEntry::create_action(|_| DialogMessage::RequestAboutGraphiteDialog.into()),
						..MenuBarEntry::default()
					}],
					vec![
						MenuBarEntry {
							label: "Donate to Graphite".into(),
							icon: Some("Heart".into()),
							action: MenuBarEntry::create_action(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://graphite.rs/donate/".into(),
								}
								.into()
							}),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "User Manual".into(),
							icon: Some("UserManual".into()),
							action: MenuBarEntry::create_action(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://graphite.rs/learn/".into(),
								}
								.into()
							}),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Report a Bug".into(),
							icon: Some("Bug".into()),
							action: MenuBarEntry::create_action(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://github.com/GraphiteEditor/Graphite/issues/new".into(),
								}
								.into()
							}),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Visit on GitHub".into(),
							icon: Some("Website".into()),
							action: MenuBarEntry::create_action(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://github.com/GraphiteEditor/Graphite".into(),
								}
								.into()
							}),
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Developer Debug".into(),
						icon: Some("Code".into()),
						action: MenuBarEntry::no_action(),
						children: MenuBarEntryChildren(vec![
							vec![MenuBarEntry {
								label: "Reset Nodes to Definitions on Open".into(),
								icon: Some(if reset_node_definitions_on_open { "CheckboxChecked" } else { "CheckboxUnchecked" }.into()),
								action: MenuBarEntry::create_action(|_| PortfolioMessage::ToggleResetNodesToDefinitionsOnOpen.into()),
								..MenuBarEntry::default()
							}],
							vec![
								MenuBarEntry {
									label: "Print Trace Logs".into(),
									icon: Some(if log::max_level() == log::LevelFilter::Trace { "CheckboxChecked" } else { "CheckboxUnchecked" }.into()),
									action: MenuBarEntry::create_action(|_| DebugMessage::ToggleTraceLogs.into()),
									..MenuBarEntry::default()
								},
								MenuBarEntry {
									label: "Print Messages: Off".into(),
									icon: message_logging_verbosity_off.then_some("SmallDot".into()),
									shortcut: action_keys!(DebugMessageDiscriminant::MessageOff),
									action: MenuBarEntry::create_action(|_| DebugMessage::MessageOff.into()),
									..MenuBarEntry::default()
								},
								MenuBarEntry {
									label: "Print Messages: Only Names".into(),
									icon: message_logging_verbosity_names.then_some("SmallDot".into()),
									shortcut: action_keys!(DebugMessageDiscriminant::MessageNames),
									action: MenuBarEntry::create_action(|_| DebugMessage::MessageNames.into()),
									..MenuBarEntry::default()
								},
								MenuBarEntry {
									label: "Print Messages: Full Contents".into(),
									icon: message_logging_verbosity_contents.then_some("SmallDot".into()),
									shortcut: action_keys!(DebugMessageDiscriminant::MessageContents),
									action: MenuBarEntry::create_action(|_| DebugMessage::MessageContents.into()),
									..MenuBarEntry::default()
								},
							],
							vec![MenuBarEntry {
								label: "Trigger a Crash".into(),
								icon: Some("Warning".into()),
								action: MenuBarEntry::create_action(|_| panic!()),
								..MenuBarEntry::default()
							}],
						]),
						..MenuBarEntry::default()
					}],
				]),
			),
		];
		Layout::MenuLayout(MenuLayout::new(menu_bar_entries))
	}
}
