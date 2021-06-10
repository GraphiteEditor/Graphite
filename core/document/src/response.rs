use crate::LayerId;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum DocumentResponse {
	DocumentChanged,
	FolderChanged { path: Vec<LayerId> },
	SelectLayer { path: Vec<LayerId> },
}

impl fmt::Display for DocumentResponse {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let name = match self {
			DocumentResponse::DocumentChanged { .. } => "DocumentChanged",
			DocumentResponse::FolderChanged { .. } => "FolderChanged",
			DocumentResponse::SelectLayer { .. } => "SelectLayer",
		};

		formatter.write_str(name)
	}
}
