use crate::LayerId;

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum DocumentResponse {
	/// For the purposes of rendering, this triggers a re-render of the entire document.
	DocumentChanged,
	/// Triggers an update of the layer in the layer panel.
	LayerChanged { path: Vec<LayerId> },
}

impl fmt::Display for DocumentResponse {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			DocumentResponse::DocumentChanged { .. } => write!(f, "DocumentChanged"),
			DocumentResponse::LayerChanged { .. } => write!(f, "LayerChanged"),
		}
	}
}
