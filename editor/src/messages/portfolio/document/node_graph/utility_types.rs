use graph_craft::document::value::TaggedValue;
use graph_craft::document::NodeId;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FrontendGraphDataType {
	#[default]
	#[serde(rename = "general")]
	General,
	#[serde(rename = "raster")]
	Raster,
	#[serde(rename = "color")]
	Color,
	#[serde(rename = "general")]
	Text,
	#[serde(rename = "vector")]
	Subpath,
	#[serde(rename = "number")]
	Number,
	#[serde(rename = "general")]
	Boolean,
	/// Refers to the mathematical vector, with direction and magnitude.
	#[serde(rename = "number")]
	Vector,
	#[serde(rename = "raster")]
	GraphicGroup,
	#[serde(rename = "artboard")]
	Artboard,
	#[serde(rename = "color")]
	Palette,
}

impl FrontendGraphDataType {
	pub const fn with_tagged_value(value: &TaggedValue) -> Self {
		match value {
			TaggedValue::String(_) => Self::Text,
			TaggedValue::F32(_) | TaggedValue::F64(_) | TaggedValue::U32(_) | TaggedValue::DAffine2(_) => Self::Number,
			TaggedValue::Bool(_) => Self::Boolean,
			TaggedValue::DVec2(_) | TaggedValue::IVec2(_) => Self::Vector,
			TaggedValue::Image(_) => Self::Raster,
			TaggedValue::ImageFrame(_) => Self::Raster,
			TaggedValue::Color(_) => Self::Color,
			TaggedValue::RcSubpath(_) | TaggedValue::Subpaths(_) | TaggedValue::VectorData(_) => Self::Subpath,
			TaggedValue::GraphicGroup(_) => Self::GraphicGroup,
			TaggedValue::Artboard(_) => Self::Artboard,
			TaggedValue::Palette(_) => Self::Palette,
			_ => Self::General,
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphInput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	#[serde(rename = "resolvedType")]
	pub resolved_type: Option<String>,
	pub connected: Option<NodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphOutput {
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub name: String,
	#[serde(rename = "resolvedType")]
	pub resolved_type: Option<String>,
	pub connected: Option<NodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNode {
	pub id: graph_craft::document::NodeId,
	#[serde(rename = "isLayer")]
	pub is_layer: bool,
	pub alias: String,
	pub name: String,
	#[serde(rename = "primaryInput")]
	pub primary_input: Option<FrontendGraphInput>,
	#[serde(rename = "exposedInputs")]
	pub exposed_inputs: Vec<FrontendGraphInput>,
	#[serde(rename = "primaryOutput")]
	pub primary_output: Option<FrontendGraphOutput>,
	#[serde(rename = "exposedOutputs")]
	pub exposed_outputs: Vec<FrontendGraphOutput>,
	pub position: (i32, i32),
	pub visible: bool,
	pub locked: bool,
	pub previewed: bool,
	pub errors: Option<String>,
}

// (link_start, link_end, link_end_input_index)
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeLink {
	#[serde(rename = "linkStart")]
	pub link_start: NodeId,
	#[serde(rename = "linkStartOutputIndex")]
	pub link_start_output_index: usize,
	#[serde(rename = "linkEnd")]
	pub link_end: NodeId,
	#[serde(rename = "linkEndInputIndex")]
	pub link_end_input_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeType {
	pub name: String,
	pub category: String,
}

impl FrontendNodeType {
	pub fn new(name: &'static str, category: &'static str) -> Self {
		Self {
			name: name.to_string(),
			category: category.to_string(),
		}
	}
}
