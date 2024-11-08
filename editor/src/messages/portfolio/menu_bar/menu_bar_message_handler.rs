use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::prelude::*;

pub struct MenuBarMessageData {
	pub has_active_document: bool,
	pub rulers_visible: bool,
	pub node_graph_open: bool,
	pub has_selected_nodes: bool,
	pub has_selected_layers: bool,
}

#[derive(Debug, Clone, Default)]
pub struct MenuBarMessageHandler {
	has_active_document: bool,
	rulers_visible: bool,
	node_graph_open: bool,
	has_selected_nodes: bool,
	has_selected_layers: bool,
}

impl MessageHandler<MenuBarMessage, MenuBarMessageData> for MenuBarMessageHandler {
	fn process_message(&mut self, message: MenuBarMessage, responses: &mut VecDeque<Message>, data: MenuBarMessageData) {
		let MenuBarMessageData {
			has_active_document,
			rulers_visible,
			node_graph_open,
			has_selected_nodes,
			has_selected_layers,
		} = data;
		self.has_active_document = has_active_document;
		self.rulers_visible = rulers_visible;
		self.node_graph_open = node_graph_open;
		self.has_selected_nodes = has_selected_nodes;
		self.has_selected_layers = has_selected_layers;

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
							shortcut: action_keys!(PortfolioMessageDiscriminant::CloseActiveDocumentWithConfirmation),
							action: MenuBarEntry::create_action(|_| PortfolioMessage::CloseActiveDocumentWithConfirmation.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Close All".into(),
							shortcut: action_keys!(PortfolioMessageDiscriminant::CloseAllDocumentsWithConfirmation),
							action: MenuBarEntry::create_action(|_| PortfolioMessage::CloseAllDocumentsWithConfirmation.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Save".into(),
						shortcut: action_keys!(DocumentMessageDiscriminant::SaveDocument),
						action: MenuBarEntry::create_action(|_| DocumentMessage::SaveDocument.into()),
						disabled: no_active_document,
						..MenuBarEntry::default()
					}],
					vec![
						MenuBarEntry {
							label: "Import…".into(),
							shortcut: action_keys!(PortfolioMessageDiscriminant::Import),
							action: MenuBarEntry::create_action(|_| PortfolioMessage::Import.into()),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Export…".into(),
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
							shortcut: action_keys!(DocumentMessageDiscriminant::Undo),
							action: MenuBarEntry::create_action(|_| DocumentMessage::Undo.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Redo".into(),
							shortcut: action_keys!(DocumentMessageDiscriminant::Redo),
							action: MenuBarEntry::create_action(|_| DocumentMessage::Redo.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Cut".into(),
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
				]),
			),
			MenuBarEntry::new_root(
				"Layer".into(),
				no_active_document,
				MenuBarEntryChildren(vec![
					vec![
						MenuBarEntry {
							label: "Select All".into(),
							shortcut: action_keys!(DocumentMessageDiscriminant::SelectAllLayers),
							action: MenuBarEntry::create_action(|_| DocumentMessage::SelectAllLayers.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Deselect All".into(),
							shortcut: action_keys!(DocumentMessageDiscriminant::DeselectAllLayers),
							action: MenuBarEntry::create_action(|_| DocumentMessage::DeselectAllLayers.into()),
							disabled: no_active_document || !has_selected_nodes,
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Previous Selection".into(),
							shortcut: action_keys!(DocumentMessageDiscriminant::SelectionStepBack),
							action: MenuBarEntry::create_action(|_| DocumentMessage::SelectionStepBack.into()),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Next Selection".into(),
							shortcut: action_keys!(DocumentMessageDiscriminant::SelectionStepForward),
							action: MenuBarEntry::create_action(|_| DocumentMessage::SelectionStepForward.into()),
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Delete Selected".into(),
						icon: Some("Trash".into()),
						shortcut: action_keys!(DocumentMessageDiscriminant::DeleteSelectedLayers),
						action: MenuBarEntry::create_action(|_| DocumentMessage::DeleteSelectedLayers.into()),
						disabled: no_active_document || !has_selected_nodes,
						..MenuBarEntry::default()
					}],
					vec![
						MenuBarEntry {
							label: "Grab Selected".into(),
							shortcut: action_keys!(TransformLayerMessageDiscriminant::BeginGrab),
							action: MenuBarEntry::create_action(|_| TransformLayerMessage::BeginGrab.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Rotate Selected".into(),
							shortcut: action_keys!(TransformLayerMessageDiscriminant::BeginRotate),
							action: MenuBarEntry::create_action(|_| TransformLayerMessage::BeginRotate.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Scale Selected".into(),
							shortcut: action_keys!(TransformLayerMessageDiscriminant::BeginScale),
							action: MenuBarEntry::create_action(|_| TransformLayerMessage::BeginScale.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Order".into(),
						action: MenuBarEntry::no_action(),
						disabled: no_active_document || !has_selected_layers,
						children: MenuBarEntryChildren(vec![vec![
							MenuBarEntry {
								label: "Raise To Front".into(),
								shortcut: action_keys!(DocumentMessageDiscriminant::SelectedLayersRaiseToFront),
								action: MenuBarEntry::create_action(|_| DocumentMessage::SelectedLayersRaiseToFront.into()),
								disabled: no_active_document || !has_selected_layers,
								..MenuBarEntry::default()
							},
							MenuBarEntry {
								label: "Raise".into(),
								shortcut: action_keys!(DocumentMessageDiscriminant::SelectedLayersRaise),
								action: MenuBarEntry::create_action(|_| DocumentMessage::SelectedLayersRaise.into()),
								disabled: no_active_document || !has_selected_layers,
								..MenuBarEntry::default()
							},
							MenuBarEntry {
								label: "Lower".into(),
								shortcut: action_keys!(DocumentMessageDiscriminant::SelectedLayersLower),
								action: MenuBarEntry::create_action(|_| DocumentMessage::SelectedLayersLower.into()),
								disabled: no_active_document || !has_selected_layers,
								..MenuBarEntry::default()
							},
							MenuBarEntry {
								label: "Lower to Back".into(),
								shortcut: action_keys!(DocumentMessageDiscriminant::SelectedLayersLowerToBack),
								action: MenuBarEntry::create_action(|_| DocumentMessage::SelectedLayersLowerToBack.into()),
								disabled: no_active_document || !has_selected_layers,
								..MenuBarEntry::default()
							},
						]]),
						..MenuBarEntry::default()
					}],
				]),
			),
			MenuBarEntry::new_root(
				"Document".into(),
				no_active_document,
				MenuBarEntryChildren(vec![vec![MenuBarEntry {
					label: "Clear Artboards".into(),
					action: MenuBarEntry::create_action(|_| DocumentMessage::ClearArtboards.into()),
					disabled: no_active_document,
					..MenuBarEntry::default()
				}]]),
			),
			MenuBarEntry::new_root(
				"View".into(),
				no_active_document,
				MenuBarEntryChildren(vec![
					vec![
						MenuBarEntry {
							label: "Tilt".into(),
							shortcut: action_keys!(NavigationMessageDiscriminant::BeginCanvasTilt),
							action: MenuBarEntry::create_action(|_| NavigationMessage::BeginCanvasTilt { was_dispatched_from_menu: true }.into()),
							disabled: no_active_document || node_graph_open,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Reset Tilt".into(),
							shortcut: action_keys!(NavigationMessageDiscriminant::CanvasTiltSet),
							action: MenuBarEntry::create_action(|_| NavigationMessage::CanvasTiltSet { angle_radians: 0.into() }.into()),
							disabled: no_active_document || node_graph_open,
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
					],
					vec![
						MenuBarEntry {
							label: "Zoom to Fit Selection".into(),
							shortcut: action_keys!(NavigationMessageDiscriminant::FitViewportToSelection),
							action: MenuBarEntry::create_action(|_| NavigationMessage::FitViewportToSelection.into()),
							disabled: no_active_document || !has_selected_layers,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Zoom to Fit All".into(),
							shortcut: action_keys!(DocumentMessageDiscriminant::ZoomCanvasToFitAll),
							action: MenuBarEntry::create_action(|_| DocumentMessage::ZoomCanvasToFitAll.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Zoom to 100%".into(),
							shortcut: action_keys!(DocumentMessageDiscriminant::ZoomCanvasTo100Percent),
							action: MenuBarEntry::create_action(|_| DocumentMessage::ZoomCanvasTo100Percent.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Zoom to 200%".into(),
							shortcut: action_keys!(DocumentMessageDiscriminant::ZoomCanvasTo200Percent),
							action: MenuBarEntry::create_action(|_| DocumentMessage::ZoomCanvasTo200Percent.into()),
							disabled: no_active_document,
							..MenuBarEntry::default()
						},
					],
					vec![MenuBarEntry {
						label: "Rulers".into(),
						icon: Some(if self.rulers_visible { "CheckboxChecked" } else { "CheckboxUnchecked" }.into()),
						shortcut: action_keys!(PortfolioMessageDiscriminant::ToggleRulers),
						action: MenuBarEntry::create_action(|_| PortfolioMessage::ToggleRulers.into()),
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
					vec![MenuBarEntry {
						label: "User Manual".into(),
						action: MenuBarEntry::create_action(|_| {
							FrontendMessage::TriggerVisitLink {
								url: "https://graphite.rs/learn/".into(),
							}
							.into()
						}),
						..MenuBarEntry::default()
					}],
					vec![
						MenuBarEntry {
							label: "Report a Bug".into(),
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
							action: MenuBarEntry::create_action(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://github.com/GraphiteEditor/Graphite".into(),
								}
								.into()
							}),
							..MenuBarEntry::default()
						},
					],
					vec![
						MenuBarEntry {
							label: "Debug: Print Messages".into(),
							action: MenuBarEntry::no_action(),
							children: MenuBarEntryChildren(vec![vec![
								MenuBarEntry {
									label: "Off".into(),
									// icon: Some("Checkmark".into()), // TODO: Find a way to set this icon on the active mode
									shortcut: action_keys!(DebugMessageDiscriminant::MessageOff),
									action: MenuBarEntry::create_action(|_| DebugMessage::MessageOff.into()),
									..MenuBarEntry::default()
								},
								MenuBarEntry {
									label: "Only Names".into(),
									shortcut: action_keys!(DebugMessageDiscriminant::MessageNames),
									action: MenuBarEntry::create_action(|_| DebugMessage::MessageNames.into()),
									..MenuBarEntry::default()
								},
								MenuBarEntry {
									label: "Full Contents".into(),
									shortcut: action_keys!(DebugMessageDiscriminant::MessageContents),
									action: MenuBarEntry::create_action(|_| DebugMessage::MessageContents.into()),
									..MenuBarEntry::default()
								},
							]]),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Debug: Print Trace Logs".into(),
							icon: Some(if log::max_level() == log::LevelFilter::Trace { "CheckboxChecked" } else { "CheckboxUnchecked" }.into()),
							shortcut: action_keys!(DebugMessageDiscriminant::ToggleTraceLogs),
							action: MenuBarEntry::create_action(|_| DebugMessage::ToggleTraceLogs.into()),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Debug: Print Document".into(),
							shortcut: action_keys!(DocumentMessageDiscriminant::DebugPrintDocument),
							action: MenuBarEntry::create_action(|_| DocumentMessage::DebugPrintDocument.into()),
							..MenuBarEntry::default()
						},
						MenuBarEntry {
							label: "Debug: Panic (DANGER)".into(),
							action: MenuBarEntry::create_action(|_| panic!()),
							..MenuBarEntry::default()
						},
					],
				]),
			),
		];
		Layout::MenuLayout(MenuLayout::new(menu_bar_entries))
	}
}
