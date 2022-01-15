use crate::LayerId;

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum DocumentResponse {
	DocumentChanged,
	FolderChanged { path: Vec<LayerId> },
	CreatedLayer { path: Vec<LayerId> },
	DeletedLayer { path: Vec<LayerId> },
	LayerChanged { path: Vec<LayerId> },
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
