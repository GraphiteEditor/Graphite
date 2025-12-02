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
	pub message_logging_verbosity: MessageLoggingVerbosity,
	pub reset_node_definitions_on_open: bool,
	pub make_path_editable_is_allowed: bool,
	pub data_panel_open: bool,
	pub layers_panel_open: bool,
	pub properties_panel_open: bool,
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

		let about = MenuListEntry::new("About Graphite…")
			.label({
				#[cfg(not(target_os = "macos"))]
				{
					"About Graphite…"
				}
				#[cfg(target_os = "macos")]
				{
					"About Graphite"
				}
			})
			.icon("GraphiteLogo")
			.on_commit(|_| DialogMessage::RequestAboutGraphiteDialog.into());
		let preferences = MenuListEntry::new("Preferences…")
			.label("Preferences…")
			.icon("Settings")
			.shortcut_keys(action_keys!(DialogMessageDiscriminant::RequestPreferencesDialog))
			.on_commit(|_| DialogMessage::RequestPreferencesDialog.into());

		let menu_bar_buttons = vec![
			#[cfg(not(target_os = "macos"))]
			TextButton::new("Graphite")
				.flush(true)
				.label("")
				.icon(Some("GraphiteLogo".into()))
				.on_commit(|_| FrontendMessage::TriggerVisitLink { url: "https://graphite.rs".into() }.into())
				.widget_holder(),
			#[cfg(target_os = "macos")]
			TextButton::new("Graphite")
				.flush(true)
				.label("")
				.menu_list_children(vec![
					vec![about],
					vec![preferences],
					vec![
						MenuListEntry::new("Hide Graphite")
							.label("Hide Graphite")
							.shortcut_keys(action_keys!(AppWindowMessageDiscriminant::Hide))
							.on_commit(|_| AppWindowMessage::Hide.into()),
						MenuListEntry::new("Hide Others")
							.label("Hide Others")
							.shortcut_keys(action_keys!(AppWindowMessageDiscriminant::HideOthers))
							.on_commit(|_| AppWindowMessage::HideOthers.into()),
						MenuListEntry::new("Show All")
							.label("Show All")
							.shortcut_keys(action_keys!(AppWindowMessageDiscriminant::ShowAll))
							.on_commit(|_| AppWindowMessage::ShowAll.into()),
					],
					vec![
						MenuListEntry::new("Quit Graphite")
							.label("Quit Graphite")
							.shortcut_keys(action_keys!(AppWindowMessageDiscriminant::Close))
							.on_commit(|_| AppWindowMessage::Close.into()),
					],
				])
				.widget_holder(),
			TextButton::new("File")
				.flush(true)
				.label("File")
				.menu_list_children(vec![
					vec![
						MenuListEntry::new("New…")
							.label("New…")
							.icon("File")
							.on_commit(|_| DialogMessage::RequestNewDocumentDialog.into())
							.shortcut_keys(action_keys!(DialogMessageDiscriminant::RequestNewDocumentDialog)),
						MenuListEntry {
							value: "Open…".into(),
							label: "Open…".into(),
							icon: "Folder".into(),
							shortcut_keys: action_keys!(PortfolioMessageDiscriminant::OpenDocument),
							on_commit: WidgetCallback::new(|_| PortfolioMessage::OpenDocument.into()),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Open Demo Artwork…".into(),
							label: "Open Demo Artwork…".into(),
							icon: "Image".into(),
							on_commit: WidgetCallback::new(|_| DialogMessage::RequestDemoArtworkDialog.into()),
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Close".into(),
							label: "Close".into(),
							icon: "Close".into(),
							shortcut_keys: action_keys!(PortfolioMessageDiscriminant::CloseActiveDocumentWithConfirmation),
							on_commit: WidgetCallback::new(|_| PortfolioMessage::CloseActiveDocumentWithConfirmation.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Close All".into(),
							label: "Close All".into(),
							icon: "CloseAll".into(),
							shortcut_keys: action_keys!(PortfolioMessageDiscriminant::CloseAllDocumentsWithConfirmation),
							on_commit: WidgetCallback::new(|_| PortfolioMessage::CloseAllDocumentsWithConfirmation.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Save".into(),
							label: "Save".into(),
							icon: "Save".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::SaveDocument),
							on_commit: WidgetCallback::new(|_| DocumentMessage::SaveDocument.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
						#[cfg(not(target_family = "wasm"))]
						MenuListEntry {
							value: "Save As…".into(),
							label: "Save As…".into(),
							icon: "Save".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::SaveDocumentAs),
							on_commit: WidgetCallback::new(|_| DocumentMessage::SaveDocumentAs.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Import…".into(),
							label: "Import…".into(),
							icon: "FileImport".into(),
							shortcut_keys: action_keys!(PortfolioMessageDiscriminant::Import),
							on_commit: WidgetCallback::new(|_| PortfolioMessage::Import.into()),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Export…".into(),
							label: "Export…".into(),
							icon: "FileExport".into(),
							shortcut_keys: action_keys!(DialogMessageDiscriminant::RequestExportDialog),
							on_commit: WidgetCallback::new(|_| DialogMessage::RequestExportDialog.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
					],
					#[cfg(not(target_os = "macos"))]
					vec![preferences],
				])
				.widget_holder(),
			TextButton::new("Edit")
				.flush(true)
				.label("Edit")
				.menu_list_children(vec![
					vec![
						MenuListEntry {
							value: "Undo".into(),
							label: "Undo".into(),
							icon: "HistoryUndo".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::Undo),
							on_commit: WidgetCallback::new(|_| DocumentMessage::Undo.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Redo".into(),
							label: "Redo".into(),
							icon: "HistoryRedo".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::Redo),
							on_commit: WidgetCallback::new(|_| DocumentMessage::Redo.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Cut".into(),
							label: "Cut".into(),
							icon: "Cut".into(),
							shortcut_keys: action_keys!(PortfolioMessageDiscriminant::Cut),
							on_commit: WidgetCallback::new(|_| PortfolioMessage::Cut { clipboard: Clipboard::Device }.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Copy".into(),
							label: "Copy".into(),
							icon: "Copy".into(),
							shortcut_keys: action_keys!(PortfolioMessageDiscriminant::Copy),
							on_commit: WidgetCallback::new(|_| PortfolioMessage::Copy { clipboard: Clipboard::Device }.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Paste".into(),
							label: "Paste".into(),
							icon: "Paste".into(),
							shortcut_keys: action_keys!(FrontendMessageDiscriminant::TriggerPaste),
							on_commit: WidgetCallback::new(|_| FrontendMessage::TriggerPaste.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Duplicate".into(),
							label: "Duplicate".into(),
							icon: "Copy".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::DuplicateSelectedLayers),
							on_commit: WidgetCallback::new(|_| DocumentMessage::DuplicateSelectedLayers.into()),
							disabled: no_active_document || !has_selected_nodes,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Delete".into(),
							label: "Delete".into(),
							icon: "Trash".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::DeleteSelectedLayers),
							on_commit: WidgetCallback::new(|_| DocumentMessage::DeleteSelectedLayers.into()),
							disabled: no_active_document || !has_selected_nodes,
							..MenuListEntry::default()
						},
					],
					vec![MenuListEntry {
						value: "Convert to Infinite Canvas".into(),
						label: "Convert to Infinite Canvas".into(),
						icon: "Artboard".into(),
						on_commit: WidgetCallback::new(|_| DocumentMessage::RemoveArtboards.into()),
						disabled: no_active_document,
						..MenuListEntry::default()
					}],
				])
				.widget_holder(),
			TextButton::new("Layer")
				.flush(true)
				.label("Layer")
				.disabled(no_active_document)
				.menu_list_children(vec![
					vec![MenuListEntry {
						value: "New".into(),
						label: "New".into(),
						icon: "NewLayer".into(),
						shortcut_keys: action_keys!(DocumentMessageDiscriminant::CreateEmptyFolder),
						on_commit: WidgetCallback::new(|_| DocumentMessage::CreateEmptyFolder.into()),
						disabled: no_active_document,
						..MenuListEntry::default()
					}],
					vec![
						MenuListEntry {
							value: "Group".into(),
							label: "Group".into(),
							icon: "Folder".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::GroupSelectedLayers),
							on_commit: WidgetCallback::new(|_| {
								DocumentMessage::GroupSelectedLayers {
									group_folder_type: GroupFolderType::Layer,
								}
								.into()
							}),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Ungroup".into(),
							label: "Ungroup".into(),
							icon: "FolderOpen".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::UngroupSelectedLayers),
							on_commit: WidgetCallback::new(|_| DocumentMessage::UngroupSelectedLayers.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Hide/Show".into(),
							label: "Hide/Show".into(),
							icon: "EyeHide".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::ToggleSelectedVisibility),
							on_commit: WidgetCallback::new(|_| DocumentMessage::ToggleSelectedVisibility.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Lock/Unlock".into(),
							label: "Lock/Unlock".into(),
							icon: "PadlockLocked".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::ToggleSelectedLocked),
							on_commit: WidgetCallback::new(|_| DocumentMessage::ToggleSelectedLocked.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Grab".into(),
							label: "Grab".into(),
							icon: "TransformationGrab".into(),
							shortcut_keys: action_keys!(TransformLayerMessageDiscriminant::BeginGrab),
							on_commit: WidgetCallback::new(|_| TransformLayerMessage::BeginGrab.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Rotate".into(),
							label: "Rotate".into(),
							icon: "TransformationRotate".into(),
							shortcut_keys: action_keys!(TransformLayerMessageDiscriminant::BeginRotate),
							on_commit: WidgetCallback::new(|_| TransformLayerMessage::BeginRotate.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Scale".into(),
							label: "Scale".into(),
							icon: "TransformationScale".into(),
							shortcut_keys: action_keys!(TransformLayerMessageDiscriminant::BeginScale),
							on_commit: WidgetCallback::new(|_| TransformLayerMessage::BeginScale.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Arrange".into(),
							label: "Arrange".into(),
							icon: "StackHollow".into(),
							disabled: no_active_document || !has_selected_layers,
							children: (vec![
								vec![
									MenuListEntry {
										value: "Raise To Front".into(),
										label: "Raise To Front".into(),
										icon: "Stack".into(),
										shortcut_keys: action_keys!(DocumentMessageDiscriminant::SelectedLayersRaiseToFront),
										on_commit: WidgetCallback::new(|_| DocumentMessage::SelectedLayersRaiseToFront.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuListEntry::default()
									},
									MenuListEntry {
										value: "Raise".into(),
										label: "Raise".into(),
										icon: "StackRaise".into(),
										shortcut_keys: action_keys!(DocumentMessageDiscriminant::SelectedLayersRaise),
										on_commit: WidgetCallback::new(|_| DocumentMessage::SelectedLayersRaise.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuListEntry::default()
									},
									MenuListEntry {
										value: "Lower".into(),
										label: "Lower".into(),
										icon: "StackLower".into(),
										shortcut_keys: action_keys!(DocumentMessageDiscriminant::SelectedLayersLower),
										on_commit: WidgetCallback::new(|_| DocumentMessage::SelectedLayersLower.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuListEntry::default()
									},
									MenuListEntry {
										value: "Lower to Back".into(),
										label: "Lower to Back".into(),
										icon: "StackBottom".into(),
										shortcut_keys: action_keys!(DocumentMessageDiscriminant::SelectedLayersLowerToBack),
										on_commit: WidgetCallback::new(|_| DocumentMessage::SelectedLayersLowerToBack.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuListEntry::default()
									},
								],
								vec![MenuListEntry {
									value: "Reverse".into(),
									label: "Reverse".into(),
									icon: "StackReverse".into(),
									on_commit: WidgetCallback::new(|_| DocumentMessage::SelectedLayersReverse.into()),
									disabled: no_active_document || !has_selected_layers,
									..MenuListEntry::default()
								}],
							]),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Align".into(),
							label: "Align".into(),
							icon: "AlignVerticalCenter".into(),
							disabled: no_active_document || !has_selected_layers,
							children: ({
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
									.map(|section| {
										section
											.into_iter()
											.map(|(axis, aggregate, icon, name)| MenuListEntry {
												value: name.into(),
												label: name.into(),
												icon: icon.into(),
												on_commit: WidgetCallback::new(move |_| DocumentMessage::AlignSelectedLayers { axis, aggregate }.into()),
												disabled: no_active_document || !has_selected_layers,
												..MenuListEntry::default()
											})
											.collect()
									})
									.collect()
							}),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Flip".into(),
							label: "Flip".into(),
							icon: "FlipVertical".into(),
							disabled: no_active_document || !has_selected_layers,
							children: (vec![{
								[(FlipAxis::X, "FlipHorizontal", "Flip Horizontal"), (FlipAxis::Y, "FlipVertical", "Flip Vertical")]
									.into_iter()
									.map(|(flip_axis, icon, name)| MenuListEntry {
										value: name.into(),
										label: name.into(),
										icon: icon.into(),
										on_commit: WidgetCallback::new(move |_| DocumentMessage::FlipSelectedLayers { flip_axis }.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuListEntry::default()
									})
									.collect()
							}]),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Turn".into(),
							label: "Turn".into(),
							icon: "TurnPositive90".into(),
							disabled: no_active_document || !has_selected_layers,
							children: (vec![{
								[(-90., "TurnNegative90", "Turn -90°"), (90., "TurnPositive90", "Turn 90°")]
									.into_iter()
									.map(|(degrees, icon, name)| MenuListEntry {
										value: name.into(),
										label: name.into(),
										icon: icon.into(),
										on_commit: WidgetCallback::new(move |_| DocumentMessage::RotateSelectedLayers { degrees }.into()),
										disabled: no_active_document || !has_selected_layers,
										..MenuListEntry::default()
									})
									.collect()
							}]),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Boolean".into(),
							label: "Boolean".into(),
							icon: "BooleanSubtractFront".into(),
							disabled: no_active_document || !has_selected_layers,
							children: (vec![{
								let list = <BooleanOperation as graphene_std::choice_type::ChoiceTypeStatic>::list();
								list.iter()
									.flat_map(|i| i.iter())
									.map(move |(operation, info)| MenuListEntry {
										value: info.label.to_string(),
										label: info.label.to_string(),
										icon: info.icon.as_ref().map(|i| i.to_string()).unwrap_or_default(),
										on_commit: WidgetCallback::new(move |_| {
											let group_folder_type = GroupFolderType::BooleanOperation(*operation);
											DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
										}),
										disabled: no_active_document || !has_selected_layers,
										..MenuListEntry::default()
									})
									.collect()
							}]),
							..MenuListEntry::default()
						},
					],
					vec![MenuListEntry {
						value: "Make Path Editable".into(),
						label: "Make Path Editable".into(),
						icon: "NodeShape".into(),
						on_commit: WidgetCallback::new(|_| NodeGraphMessage::AddPathNode.into()),
						disabled: !make_path_editable_is_allowed,
						..MenuListEntry::default()
					}],
				])
				.widget_holder(),
			TextButton::new("Select")
				.flush(true)
				.label("Select")
				.disabled(no_active_document)
				.menu_list_children(vec![
					vec![
						MenuListEntry {
							value: "Select All".into(),
							label: "Select All".into(),
							icon: "SelectAll".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::SelectAllLayers),
							on_commit: WidgetCallback::new(|_| DocumentMessage::SelectAllLayers.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Deselect All".into(),
							label: "Deselect All".into(),
							icon: "DeselectAll".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::DeselectAllLayers),
							on_commit: WidgetCallback::new(|_| DocumentMessage::DeselectAllLayers.into()),
							disabled: no_active_document || !has_selected_nodes,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Select Parent".into(),
							label: "Select Parent".into(),
							icon: "SelectParent".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::SelectParentLayer),
							on_commit: WidgetCallback::new(|_| DocumentMessage::SelectParentLayer.into()),
							disabled: no_active_document || !has_selected_nodes,
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Previous Selection".into(),
							label: "Previous Selection".into(),
							icon: "HistoryUndo".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::SelectionStepBack),
							on_commit: WidgetCallback::new(|_| DocumentMessage::SelectionStepBack.into()),
							disabled: !has_selection_history.0,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Next Selection".into(),
							label: "Next Selection".into(),
							icon: "HistoryRedo".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::SelectionStepForward),
							on_commit: WidgetCallback::new(|_| DocumentMessage::SelectionStepForward.into()),
							disabled: !has_selection_history.1,
							..MenuListEntry::default()
						},
					],
				])
				.widget_holder(),
			TextButton::new("View")
				.flush(true)
				.label("View")
				.disabled(no_active_document)
				.menu_list_children(vec![
					vec![
						MenuListEntry {
							value: "Tilt".into(),
							label: "Tilt".into(),
							icon: "Tilt".into(),
							shortcut_keys: action_keys!(NavigationMessageDiscriminant::BeginCanvasTilt),
							on_commit: WidgetCallback::new(|_| NavigationMessage::BeginCanvasTilt { was_dispatched_from_menu: true }.into()),
							disabled: no_active_document || node_graph_open,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Reset Tilt".into(),
							label: "Reset Tilt".into(),
							icon: "TiltReset".into(),
							shortcut_keys: action_keys!(NavigationMessageDiscriminant::CanvasTiltSet),
							on_commit: WidgetCallback::new(|_| NavigationMessage::CanvasTiltSet { angle_radians: 0.into() }.into()),
							disabled: no_active_document || node_graph_open || !self.canvas_tilted,
							..MenuListEntry::default()
						},
					],
					vec![
						MenuListEntry {
							value: "Zoom In".into(),
							label: "Zoom In".into(),
							icon: "ZoomIn".into(),
							shortcut_keys: action_keys!(NavigationMessageDiscriminant::CanvasZoomIncrease),
							on_commit: WidgetCallback::new(|_| NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Zoom Out".into(),
							label: "Zoom Out".into(),
							icon: "ZoomOut".into(),
							shortcut_keys: action_keys!(NavigationMessageDiscriminant::CanvasZoomDecrease),
							on_commit: WidgetCallback::new(|_| NavigationMessage::CanvasZoomDecrease { center_on_mouse: false }.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Zoom to Selection".into(),
							label: "Zoom to Selection".into(),
							icon: "FrameSelected".into(),
							shortcut_keys: action_keys!(NavigationMessageDiscriminant::FitViewportToSelection),
							on_commit: WidgetCallback::new(|_| NavigationMessage::FitViewportToSelection.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Zoom to Fit".into(),
							label: "Zoom to Fit".into(),
							icon: "FrameAll".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::ZoomCanvasToFitAll),
							on_commit: WidgetCallback::new(|_| DocumentMessage::ZoomCanvasToFitAll.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Zoom to 100%".into(),
							label: "Zoom to 100%".into(),
							icon: "Zoom1x".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::ZoomCanvasTo100Percent),
							on_commit: WidgetCallback::new(|_| DocumentMessage::ZoomCanvasTo100Percent.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Zoom to 200%".into(),
							label: "Zoom to 200%".into(),
							icon: "Zoom2x".into(),
							shortcut_keys: action_keys!(DocumentMessageDiscriminant::ZoomCanvasTo200Percent),
							on_commit: WidgetCallback::new(|_| DocumentMessage::ZoomCanvasTo200Percent.into()),
							disabled: no_active_document,
							..MenuListEntry::default()
						},
					],
					vec![MenuListEntry {
						value: "Flip".into(),
						label: "Flip".into(),
						icon: if self.canvas_flipped { "CheckboxChecked" } else { "CheckboxUnchecked" }.into(),
						shortcut_keys: action_keys!(NavigationMessageDiscriminant::CanvasFlip),
						on_commit: WidgetCallback::new(|_| NavigationMessage::CanvasFlip.into()),
						disabled: no_active_document || node_graph_open,
						..MenuListEntry::default()
					}],
					vec![MenuListEntry {
						value: "Rulers".into(),
						label: "Rulers".into(),
						icon: if self.rulers_visible { "CheckboxChecked" } else { "CheckboxUnchecked" }.into(),
						shortcut_keys: action_keys!(PortfolioMessageDiscriminant::ToggleRulers),
						on_commit: WidgetCallback::new(|_| PortfolioMessage::ToggleRulers.into()),
						disabled: no_active_document,
						..MenuListEntry::default()
					}],
				])
				.widget_holder(),
			TextButton::new("Window")
				.flush(true)
				.label("Window")
				.menu_list_children(vec![
					vec![
						MenuListEntry {
							value: "Properties".into(),
							label: "Properties".into(),
							icon: if self.properties_panel_open { "CheckboxChecked" } else { "CheckboxUnchecked" }.into(),
							shortcut_keys: action_keys!(PortfolioMessageDiscriminant::TogglePropertiesPanelOpen),
							on_commit: WidgetCallback::new(|_| PortfolioMessage::TogglePropertiesPanelOpen.into()),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Layers".into(),
							label: "Layers".into(),
							icon: if self.layers_panel_open { "CheckboxChecked" } else { "CheckboxUnchecked" }.into(),
							shortcut_keys: action_keys!(PortfolioMessageDiscriminant::ToggleLayersPanelOpen),
							on_commit: WidgetCallback::new(|_| PortfolioMessage::ToggleLayersPanelOpen.into()),
							..MenuListEntry::default()
						},
					],
					vec![MenuListEntry {
						value: "Data".into(),
						label: "Data".into(),
						icon: if self.data_panel_open { "CheckboxChecked" } else { "CheckboxUnchecked" }.into(),
						shortcut_keys: action_keys!(PortfolioMessageDiscriminant::ToggleDataPanelOpen),
						on_commit: WidgetCallback::new(|_| PortfolioMessage::ToggleDataPanelOpen.into()),
						..MenuListEntry::default()
					}],
				])
				.widget_holder(),
			TextButton::new("Help")
				.flush(true)
				.label("Help")
				.menu_list_children(vec![
					#[cfg(not(target_os = "macos"))]
					vec![about],
					vec![
						MenuListEntry {
							value: "Donate to Graphite".into(),
							label: "Donate to Graphite".into(),
							icon: "Heart".into(),
							on_commit: WidgetCallback::new(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://graphite.rs/donate/".into(),
								}
								.into()
							}),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "User Manual".into(),
							label: "User Manual".into(),
							icon: "UserManual".into(),
							on_commit: WidgetCallback::new(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://graphite.rs/learn/".into(),
								}
								.into()
							}),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Report a Bug".into(),
							label: "Report a Bug".into(),
							icon: "Bug".into(),
							on_commit: WidgetCallback::new(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://github.com/GraphiteEditor/Graphite/issues/new".into(),
								}
								.into()
							}),
							..MenuListEntry::default()
						},
						MenuListEntry {
							value: "Visit on GitHub".into(),
							label: "Visit on GitHub".into(),
							icon: "Website".into(),
							on_commit: WidgetCallback::new(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://github.com/GraphiteEditor/Graphite".into(),
								}
								.into()
							}),
							..MenuListEntry::default()
						},
					],
					vec![MenuListEntry {
						value: "Developer Debug".into(),
						label: "Developer Debug".into(),
						icon: "Code".into(),
						children: (vec![
							vec![MenuListEntry {
								value: "Reset Nodes to Definitions on Open".into(),
								label: "Reset Nodes to Definitions on Open".into(),
								icon: if reset_node_definitions_on_open { "CheckboxChecked" } else { "CheckboxUnchecked" }.into(),
								on_commit: WidgetCallback::new(|_| PortfolioMessage::ToggleResetNodesToDefinitionsOnOpen.into()),
								..MenuListEntry::default()
							}],
							vec![
								MenuListEntry {
									value: "Print Trace Logs".into(),
									label: "Print Trace Logs".into(),
									icon: if log::max_level() == log::LevelFilter::Trace { "CheckboxChecked" } else { "CheckboxUnchecked" }.into(),
									on_commit: WidgetCallback::new(|_| DebugMessage::ToggleTraceLogs.into()),
									..MenuListEntry::default()
								},
								MenuListEntry {
									value: "Print Messages: Off".into(),
									label: "Print Messages: Off".into(),
									#[cfg(not(target_os = "macos"))]
									icon: message_logging_verbosity_off.then_some("SmallDot".into()).unwrap_or_default(),
									#[cfg(target_os = "macos")]
									icon: message_logging_verbosity_off.then_some("CheckboxChecked".into()).unwrap_or_default(),
									shortcut_keys: action_keys!(DebugMessageDiscriminant::MessageOff),
									on_commit: WidgetCallback::new(|_| DebugMessage::MessageOff.into()),
									..MenuListEntry::default()
								},
								MenuListEntry {
									value: "Print Messages: Only Names".into(),
									label: "Print Messages: Only Names".into(),
									#[cfg(not(target_os = "macos"))]
									icon: message_logging_verbosity_names.then_some("SmallDot".into()).unwrap_or_default(),
									#[cfg(target_os = "macos")]
									icon: message_logging_verbosity_names.then_some("CheckboxChecked".into()).unwrap_or_default(),
									shortcut_keys: action_keys!(DebugMessageDiscriminant::MessageNames),
									on_commit: WidgetCallback::new(|_| DebugMessage::MessageNames.into()),
									..MenuListEntry::default()
								},
								MenuListEntry {
									value: "Print Messages: Full Contents".into(),
									label: "Print Messages: Full Contents".into(),
									#[cfg(not(target_os = "macos"))]
									icon: message_logging_verbosity_contents.then_some("SmallDot".into()).unwrap_or_default(),
									#[cfg(target_os = "macos")]
									icon: message_logging_verbosity_contents.then_some("CheckboxChecked".into()).unwrap_or_default(),
									shortcut_keys: action_keys!(DebugMessageDiscriminant::MessageContents),
									on_commit: WidgetCallback::new(|_| DebugMessage::MessageContents.into()),
									..MenuListEntry::default()
								},
							],
							vec![MenuListEntry {
								value: "Trigger a Crash".into(),
								label: "Trigger a Crash".into(),
								icon: "Warning".into(),
								on_commit: WidgetCallback::new(|_| panic!()),
								..MenuListEntry::default()
							}],
						]),
						..MenuListEntry::default()
					}],
				])
				.widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: menu_bar_buttons }]))
	}
}
