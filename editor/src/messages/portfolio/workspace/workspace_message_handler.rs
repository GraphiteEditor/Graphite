use crate::messages::layout::utility_types::layout_widget::LayoutTarget;
use crate::messages::portfolio::persistent_state::PersistentStateMessage;
use crate::messages::portfolio::utility_types::{PanelLayoutSubdivision, PanelType, WorkspacePanelLayout};
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct WorkspaceMessageContext {
	/// Whether the portfolio has a document selected as active.
	pub has_active_document: bool,
	/// Whether the portfolio holds no documents at all.
	pub has_no_documents: bool,
}

#[derive(Debug, Default, ExtractField)]
pub struct WorkspaceMessageHandler {
	pub panel_layout: WorkspacePanelLayout,
}

#[message_handler_data]
impl MessageHandler<WorkspaceMessage, WorkspaceMessageContext> for WorkspaceMessageHandler {
	fn process_message(&mut self, message: WorkspaceMessage, responses: &mut VecDeque<Message>, context: WorkspaceMessageContext) {
		let WorkspaceMessageContext {
			has_active_document,
			has_no_documents,
		} = context;

		match message {
			WorkspaceMessage::MoveAllPanelTabs {
				source_group,
				target_group,
				insert_index,
			} => {
				if source_group == target_group {
					return;
				}

				let Some(source_state) = self.panel_layout.panel_group(source_group) else { return };
				let tabs: Vec<PanelType> = source_state.tabs.clone();
				let source_active_tab_index = source_state.active_tab_index;
				if tabs.is_empty() {
					return;
				}

				// Validate that the target group exists before modifying the source
				if self.panel_layout.panel_group(target_group).is_none() {
					log::error!("Target panel group {target_group:?} not found");
					return;
				}

				// Destroy layouts for all moved tabs and the displaced target tab
				for &panel_type in &tabs {
					Self::destroy_panel_layouts(panel_type, responses);
				}
				if let Some(old_target_panel) = self.panel_layout.panel_group(target_group).and_then(|g| g.active_panel_type()) {
					Self::destroy_panel_layouts(old_target_panel, responses);
				}

				// Clear the source group
				if let Some(source) = self.panel_layout.panel_group_mut(source_group) {
					source.tabs.clear();
					source.active_tab_index = 0;
				}

				// Insert all tabs into the target group, preserving which tab was active in the source
				if let Some(target) = self.panel_layout.panel_group_mut(target_group) {
					let index = insert_index.min(target.tabs.len());
					target.tabs.splice(index..index, tabs.iter().copied());
					target.active_tab_index = index + source_active_tab_index.min(tabs.len().saturating_sub(1));
				}

				self.panel_layout.prune();

				responses.add(MenuBarMessage::SendLayout);
				responses.add(WorkspaceMessage::UpdatePanelsLayout);

				// Refresh the new active tab
				if let Some(panel_type) = self.panel_layout.panel_group(target_group).and_then(|g| g.active_panel_type()) {
					Self::refresh_panel_content(panel_type, has_active_document, has_no_documents, responses);
				}
			}
			WorkspaceMessage::MovePanelTab {
				source_group,
				target_group,
				insert_index,
			} => {
				if source_group == target_group {
					return;
				}

				let Some(source_state) = self.panel_layout.panel_group(source_group) else { return };
				let Some(panel_type) = source_state.active_panel_type() else { return };

				// Validate that the target group exists before modifying the source
				if self.panel_layout.panel_group(target_group).is_none() {
					log::error!("Target panel group {target_group:?} not found");
					return;
				}

				// Destroy layouts for the moved panel (so backend and frontend start in sync when it remounts)
				// and for the panel that was previously active in the target panel group (it will be displaced by the incoming tab)
				Self::destroy_panel_layouts(panel_type, responses);
				if let Some(old_target_panel) = self.panel_layout.panel_group(target_group).and_then(|g| g.active_panel_type()) {
					Self::destroy_panel_layouts(old_target_panel, responses);
				}

				// Remove from source panel group
				if let Some(source) = self.panel_layout.panel_group_mut(source_group) {
					source.tabs.retain(|&t| t != panel_type);
					source.active_tab_index = source.active_tab_index.min(source.tabs.len().saturating_sub(1));
				}

				// Insert into target panel group
				if let Some(target) = self.panel_layout.panel_group_mut(target_group) {
					let index = insert_index.min(target.tabs.len());
					target.tabs.insert(index, panel_type);
					target.active_tab_index = index;
				}

				// Remove empty panel groups from the tree
				self.panel_layout.prune();

				responses.add(MenuBarMessage::SendLayout);
				responses.add(WorkspaceMessage::UpdatePanelsLayout);

				// Refresh the moved panel's content in its new location
				Self::refresh_panel_content(panel_type, has_active_document, has_no_documents, responses);

				// Refresh the source panel group's newly active tab (if any remain) so it's not left stale
				if let Some(new_source_active) = self.panel_layout.panel_group(source_group).and_then(|g| g.active_panel_type()) {
					Self::destroy_panel_layouts(new_source_active, responses);
					Self::refresh_panel_content(new_source_active, has_active_document, has_no_documents, responses);
				}
			}
			WorkspaceMessage::ReorderPanelGroupTab { group, old_index, new_index } => {
				let Some(group_state) = self.panel_layout.panel_group_mut(group) else { return };

				if old_index < group_state.tabs.len() && new_index < group_state.tabs.len() && old_index != new_index {
					let tab = group_state.tabs.remove(old_index);
					group_state.tabs.insert(new_index, tab);

					// Keep the active tab following the reorder
					if group_state.active_tab_index == old_index {
						group_state.active_tab_index = new_index;
					} else if old_index < group_state.active_tab_index && new_index >= group_state.active_tab_index {
						group_state.active_tab_index = group_state.active_tab_index.saturating_sub(1);
					} else if old_index > group_state.active_tab_index && new_index <= group_state.active_tab_index {
						group_state.active_tab_index += 1;
					}

					responses.add(WorkspaceMessage::UpdatePanelsLayout);
				}
			}
			WorkspaceMessage::SetPanelGroupActiveTab { group, tab_index } => {
				let Some(group_state) = self.panel_layout.panel_group(group) else { return };
				if tab_index < group_state.tabs.len() && tab_index != group_state.active_tab_index {
					// Destroy layouts for the old and new panels so the backend's diffing state is in sync with the frontend's fresh mount
					if let Some(old_panel_type) = group_state.active_panel_type() {
						Self::destroy_panel_layouts(old_panel_type, responses);
					}
					let new_panel_type = group_state.tabs[tab_index];
					Self::destroy_panel_layouts(new_panel_type, responses);

					// Update the active tab index for the panel
					if let Some(group_state) = self.panel_layout.panel_group_mut(group) {
						group_state.active_tab_index = tab_index;
					}

					// Send the layout update first so the frontend mounts the new panel component before it receives content
					responses.add(WorkspaceMessage::UpdatePanelsLayout);

					if let Some(panel_type) = self.panel_layout.panel_group(group).and_then(|g| g.active_panel_type()) {
						Self::refresh_panel_content(panel_type, has_active_document, has_no_documents, responses);
					}
				}
			}
			WorkspaceMessage::SplitPanelGroup {
				target_group,
				direction,
				tabs,
				active_tab_index,
			} => {
				// Destroy layouts for the dragged tabs and the target group's active panel (it may get remounted by the frontend)
				for &panel_type in &tabs {
					Self::destroy_panel_layouts(panel_type, responses);
				}
				if let Some(target_active) = self.panel_layout.panel_group(target_group).and_then(|g| g.active_panel_type()) {
					Self::destroy_panel_layouts(target_active, responses);
				}

				// Preserve the source panel's visual weight at its new location
				let source_slot_size = self.panel_layout.find_source_slot_size(&tabs);

				// The other panel groups that the dragged tabs leave, since the target group is refreshed regardless
				let mut source_groups = Vec::new();
				for &panel_type in &tabs {
					if let Some(group) = self.panel_layout.find_panel(panel_type)
						&& group != target_group
						&& !source_groups.contains(&group)
					{
						source_groups.push(group);
					}
				}

				// Remove the dragged tabs from their current panel groups (without pruning, so the target group survives)
				for &panel_type in &tabs {
					self.remove_panel_from_layout(panel_type);
				}

				// Create the new panel group adjacent to the target, then prune empty groups
				let Some(new_id) = self.panel_layout.split_panel_group(target_group, direction, tabs.clone(), active_tab_index, source_slot_size) else {
					log::error!("Failed to insert split adjacent to panel group {target_group:?}");
					return;
				};
				self.panel_layout.prune();

				responses.add(MenuBarMessage::SendLayout);
				responses.add(WorkspaceMessage::UpdatePanelsLayout);

				// Refresh the new panel group's active tab
				if let Some(panel_type) = self.panel_layout.panel_group(new_id).and_then(|g| g.active_panel_type()) {
					Self::refresh_panel_content(panel_type, has_active_document, has_no_documents, responses);
				}

				// Refresh the target group's active panel since its component may have been remounted
				if let Some(target_active) = self.panel_layout.panel_group(target_group).and_then(|g| g.active_panel_type()) {
					Self::refresh_panel_content(target_active, has_active_document, has_no_documents, responses);
				}

				// Refresh each source panel group's newly active tab (if any remain) so it's not left stale
				for source_group in source_groups {
					if let Some(new_source_active) = self.panel_layout.panel_group(source_group).and_then(|g| g.active_panel_type()) {
						Self::destroy_panel_layouts(new_source_active, responses);
						Self::refresh_panel_content(new_source_active, has_active_document, has_no_documents, responses);
					}
				}
			}
			WorkspaceMessage::ToggleFocusDocument => {
				self.panel_layout.focus_document = !self.panel_layout.focus_document;

				// Destroy or refresh non-document panel layouts based on focus mode
				for &panel_type in PanelType::non_document_panels() {
					if self.panel_layout.is_panel_present(panel_type) {
						if self.panel_layout.focus_document {
							Self::destroy_panel_layouts(panel_type, responses);
						} else {
							Self::refresh_panel_content(panel_type, has_active_document, has_no_documents, responses);
						}
					}
				}

				responses.add(MenuBarMessage::SendLayout);
				responses.add(WorkspaceMessage::UpdatePanelsLayout);
			}
			WorkspaceMessage::TogglePropertiesPanelOpen => {
				if self.panel_layout.focus_document {
					return;
				}

				let panel_type = PanelType::Properties;
				self.toggle_dockable_panel(panel_type, has_active_document, has_no_documents, responses);
			}
			WorkspaceMessage::ToggleLayersPanelOpen => {
				if self.panel_layout.focus_document {
					return;
				}

				let panel_type = PanelType::Layers;
				self.toggle_dockable_panel(panel_type, has_active_document, has_no_documents, responses);
			}
			WorkspaceMessage::ToggleDataPanelOpen => {
				if self.panel_layout.focus_document {
					return;
				}

				let panel_type = PanelType::Data;
				self.toggle_dockable_panel(panel_type, has_active_document, has_no_documents, responses);
			}
			WorkspaceMessage::UpdatePanelsLayout => {
				let panel_layout = match self.panel_layout.focus_document {
					true => self.panel_layout.document_only_layout(),
					false => self.panel_layout.clone(),
				};
				responses.add(FrontendMessage::UpdateWorkspacePanelLayout { panel_layout });
				responses.add(PersistentStateMessage::WriteState);
			}
			WorkspaceMessage::ResetWorkspaceLayout => {
				// Destroy layouts for all currently visible non-document panels
				for &panel_type in PanelType::non_document_panels() {
					if self.panel_layout.is_panel_present(panel_type) {
						Self::destroy_panel_layouts(panel_type, responses);
					}
				}

				// Replace layout with the default and recalculate sizes
				self.panel_layout = WorkspacePanelLayout::default();
				self.panel_layout.recalculate_default_sizes();

				// Refresh all visible panels since the layout has been completely replaced
				for group_id in self.panel_layout.root.all_group_ids() {
					if let Some(panel_type) = self.panel_layout.panel_group(group_id).and_then(|g| g.active_panel_type()) {
						Self::refresh_panel_content(panel_type, has_active_document, has_no_documents, responses);
					}
				}

				responses.add(WorkspaceMessage::UpdatePanelsLayout);
				responses.add(MenuBarMessage::SendLayout);
			}
			WorkspaceMessage::SetPanelGroupSizes { split_path, sizes } => {
				// Walk the tree to the target split node using the path
				let mut node = &mut self.panel_layout.root;
				for &index in &split_path {
					let PanelLayoutSubdivision::Split { children } = node else { return };
					let Some(child) = children.get_mut(index) else { return };
					node = &mut child.subdivision;
				}

				// Apply the new sizes to the split's children
				if let PanelLayoutSubdivision::Split { children } = node {
					for (child, &size) in children.iter_mut().zip(sizes.iter()) {
						child.size = size;
					}
				}

				responses.add(WorkspaceMessage::UpdatePanelsLayout);
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(WorkspaceMessageDiscriminant;
			ToggleFocusDocument,
		);

		// Extend with actions that are disabled when focusing the document
		if !self.panel_layout.focus_document {
			common.extend(actions!(WorkspaceMessageDiscriminant;
				TogglePropertiesPanelOpen,
				ToggleLayersPanelOpen,
				ToggleDataPanelOpen,
			));
		}

		common
	}
}

impl WorkspaceMessageHandler {
	/// Remove a dockable panel type from whichever panel group currently contains it. Does not prune empty groups.
	fn remove_panel_from_layout(&mut self, panel_type: PanelType) {
		// Save the panel's current position so it can be restored there later
		self.panel_layout.save_panel_position(panel_type);

		if let Some(group_id) = self.panel_layout.find_panel(panel_type)
			&& let Some(group) = self.panel_layout.panel_group_mut(group_id)
		{
			group.tabs.retain(|&t| t != panel_type);
			group.active_tab_index = group.active_tab_index.min(group.tabs.len().saturating_sub(1));
		}
	}

	/// Toggle a dockable panel on or off. When toggling off, refresh the newly active tab in its panel group (if any).
	fn toggle_dockable_panel(&mut self, panel_type: PanelType, has_active_document: bool, has_no_documents: bool, responses: &mut VecDeque<Message>) {
		if let Some(group_id) = self.panel_layout.find_panel(panel_type) {
			// Panel is present, remove it
			let was_visible = self.panel_layout.panel_group(group_id).is_some_and(|g| g.is_visible(panel_type));
			Self::destroy_panel_layouts(panel_type, responses);
			self.remove_panel_from_layout(panel_type);
			self.panel_layout.prune();

			// If the removed panel was the active tab, refresh whichever panel is now active in that panel group
			if was_visible && let Some(new_active) = self.panel_layout.panel_group(group_id).and_then(|g| g.active_panel_type()) {
				Self::destroy_panel_layouts(new_active, responses);
				Self::refresh_panel_content(new_active, has_active_document, has_no_documents, responses);
			}
		} else {
			// Panel is not present, restore it to its default position in the layout tree
			self.panel_layout.restore_panel(panel_type);
			self.panel_layout.prune();
			Self::refresh_panel_content(panel_type, has_active_document, has_no_documents, responses);
		}

		responses.add(MenuBarMessage::SendLayout);
		responses.add(WorkspaceMessage::UpdatePanelsLayout);
	}

	/// Destroy the stored layout for a panel that is no longer the active tab.
	/// This resets the backend's diffing state so it won't try to send updates to a frontend component that has been unmounted.
	fn destroy_panel_layouts(panel_type: PanelType, responses: &mut VecDeque<Message>) {
		let targets: &[LayoutTarget] = match panel_type {
			PanelType::Properties => &[LayoutTarget::PropertiesPanel],
			PanelType::Layers => &[LayoutTarget::LayersPanelControlLeftBar, LayoutTarget::LayersPanelControlRightBar, LayoutTarget::LayersPanelBottomBar],
			PanelType::Data => &[LayoutTarget::DataPanel],
			PanelType::Document | PanelType::Welcome => return,
		};

		for &layout_target in targets {
			responses.add(LayoutMessage::DestroyLayout { layout_target });
		}
	}

	/// Trigger a content refresh for a panel that just became the active tab.
	pub(crate) fn refresh_panel_content(panel_type: PanelType, has_active_document: bool, has_no_documents: bool, responses: &mut VecDeque<Message>) {
		responses.add(NodeGraphMessage::RunDocumentGraph);

		match panel_type {
			PanelType::Properties => {
				responses.add(PropertiesPanelMessage::Refresh);
			}
			PanelType::Layers => {
				if has_active_document {
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![NodeGraphMessage::UpdateLayerPanel.into(), DocumentMessage::DocumentStructureChanged.into()],
					});
				}
			}
			PanelType::Data => {
				// The Data panel's content is populated automatically as a side effect of the graph run completing, so there's nothing to do here
			}
			PanelType::Document | PanelType::Welcome => {
				// Re-send the welcome screen buttons layout to repopulate after a remount
				if has_no_documents {
					responses.add(PortfolioMessage::RequestWelcomeScreenButtonsLayout);
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::portfolio::utility_types::DockingSplitDirection;

	#[test]
	fn splitting_a_tab_out_of_a_group_refreshes_the_tab_left_active_there() {
		let mut handler = WorkspaceMessageHandler::default();
		let context = || WorkspaceMessageContext {
			has_active_document: true,
			has_no_documents: false,
		};
		let group_of = |handler: &WorkspaceMessageHandler, panel_type| handler.panel_layout.find_panel(panel_type).expect("the default layout has this panel");

		// The Layers panel joins the Properties panel's group as its active tab
		let join = WorkspaceMessage::MovePanelTab {
			source_group: group_of(&handler, PanelType::Layers),
			target_group: group_of(&handler, PanelType::Properties),
			insert_index: 1,
		};
		handler.process_message(join, &mut VecDeque::new(), context());

		// Splitting it back out beside the document leaves the Properties panel as the active tab of the group it left
		let split = WorkspaceMessage::SplitPanelGroup {
			target_group: group_of(&handler, PanelType::Document),
			direction: DockingSplitDirection::Right,
			tabs: vec![PanelType::Layers],
			active_tab_index: 0,
		};
		let mut responses = VecDeque::new();
		handler.process_message(split, &mut responses, context());

		assert!(responses.contains(&PropertiesPanelMessage::Refresh.into()));
	}
}
