pub use super::workspace_types::*;
pub use crate::messages::prelude::*;

#[impl_message(Message, Workspace)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum WorkspaceMessage {
	AddTab { panel: PanelPath, tab: TabType, index: Option<usize> },
	DeleteTab { tab: TabPath },
	MoveTab { source: TabPath, destination: TabDestination },
	SelectTab { tab: TabPath },
	ResizeDivision { division: PanelPath, start_size: f64, end_size: f64 },

	SendLayout,
}
