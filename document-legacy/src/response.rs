use crate::LayerId;

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum DocumentResponse {
	/// For the purposes of rendering, this triggers a re-render of the entire document.
	DocumentChanged,
	FolderChanged {
		path: Vec<LayerId>,
	},
	CreatedLayer {
		path: Vec<LayerId>,
		is_selected: bool,
	},
	DeletedLayer {
		path: Vec<LayerId>,
	},
	/// Triggers an update of the layer in the layer panel.
	LayerChanged {
		path: Vec<LayerId>,
	},
	MoveSelectedLayersTo {
		folder_path: Vec<LayerId>,
		insert_index: isize,
		reverse_index: bool,
	},
	DeletedSelectedManipulatorPoints,
}

impl fmt::Display for DocumentResponse {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			DocumentResponse::DocumentChanged { .. } => write!(f, "DocumentChanged"),
			DocumentResponse::FolderChanged { .. } => write!(f, "FolderChanged"),
			DocumentResponse::CreatedLayer { .. } => write!(f, "CreatedLayer"),
			DocumentResponse::LayerChanged { .. } => write!(f, "LayerChanged"),
			DocumentResponse::DeletedLayer { .. } => write!(f, "DeleteLayer"),
			DocumentResponse::DeletedSelectedManipulatorPoints { .. } => write!(f, "DeletedSelectedManipulatorPoints"),
			DocumentResponse::MoveSelectedLayersTo { .. } => write!(f, "MoveSelectedLayersTo"),
		}
	}
}
