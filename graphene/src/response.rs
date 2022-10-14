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
	},
	DeletedLayer {
		path: Vec<LayerId>,
	},
	/// Triggers an update of the layer in the layer panel.
	LayerChanged {
		path: Vec<LayerId>,
	},
}

impl fmt::Display for DocumentResponse {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let name = match self {
			DocumentResponse::DocumentChanged { .. } => "DocumentChanged",
			DocumentResponse::FolderChanged { .. } => "FolderChanged",
			DocumentResponse::CreatedLayer { .. } => "CreatedLayer",
			DocumentResponse::LayerChanged { .. } => "LayerChanged",
			DocumentResponse::DeletedLayer { .. } => "DeleteLayer",
		};

		formatter.write_str(name)
	}
}
