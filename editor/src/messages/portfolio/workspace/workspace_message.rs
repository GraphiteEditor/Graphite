use crate::messages::portfolio::utility_types::{DockingSplitDirection, PanelGroupId, PanelType};
use crate::messages::prelude::*;

#[impl_message(Message, PortfolioMessage, Workspace)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum WorkspaceMessage {
	MoveAllPanelTabs {
		source_group: PanelGroupId,
		target_group: PanelGroupId,
		insert_index: usize,
	},
	MovePanelTab {
		source_group: PanelGroupId,
		target_group: PanelGroupId,
		insert_index: usize,
	},
	ReorderPanelGroupTab {
		group: PanelGroupId,
		old_index: usize,
		new_index: usize,
	},
	SetPanelGroupActiveTab {
		group: PanelGroupId,
		tab_index: usize,
	},
	SplitPanelGroup {
		target_group: PanelGroupId,
		direction: DockingSplitDirection,
		tabs: Vec<PanelType>,
		active_tab_index: usize,
	},
	ToggleFocusDocument,
	ToggleDataPanelOpen,
	TogglePropertiesPanelOpen,
	ToggleLayersPanelOpen,
	UpdatePanelsLayout,
	ResetWorkspaceLayout,
	SetPanelGroupSizes {
		/// Path of child indices from the root to the split node whose children's sizes are being set.
		split_path: Vec<usize>,
		/// New sizes for the children at that split node.
		sizes: Vec<f64>,
	},
}
