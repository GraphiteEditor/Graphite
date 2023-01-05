use document_legacy::layers::layer_info::LayerDataTypeDiscriminant;
use graph_craft::imaginate_input::{ImaginateMaskPaintMode, ImaginateMaskStartingFill};
use graphite_editor::messages::{
	frontend::utility_types::MouseCursorIcon,
	input_mapper::utility_types::{
		input_keyboard::{KeyDiscriminant, MouseMotion},
		misc::ActionKeys,
	},
	layout::utility_types::{
		layout_widget::{DiffUpdate, LayoutGroup, Widget},
		misc::LayoutTarget,
		widgets::{
			assist_widgets::PivotPosition,
			input_widgets::{NumberInputIncrementBehavior, NumberInputMode},
			label_widgets::{SeparatorDirection, SeparatorType},
		},
	},
	portfolio::document::node_graph::FrontendGraphDataType,
	prelude::*,
	tool::ToolMessageDiscriminant,
};

use serde_reflection::{Tracer, TracerConfig};

use graphite_editor::messages::frontend::FrontendMessage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Start the tracing session.
	let mut tracer = Tracer::new(TracerConfig::default());

	// level 0 (top level)
	tracer.trace_simple_type::<FrontendMessage>()?;
	// level 1
	tracer.trace_simple_type::<ActionKeys>()?;
	tracer.trace_simple_type::<DiffUpdate>()?;
	tracer.trace_simple_type::<FrontendGraphDataType>()?;
	tracer.trace_simple_type::<ImaginateMaskPaintMode>()?;
	tracer.trace_simple_type::<ImaginateMaskStartingFill>()?;
	tracer.trace_simple_type::<LayerDataTypeDiscriminant>()?;
	tracer.trace_simple_type::<LayoutGroup>()?;
	tracer.trace_simple_type::<LayoutTarget>()?;
	tracer.trace_simple_type::<MessageDiscriminant>()?;
	tracer.trace_simple_type::<MouseCursorIcon>()?;
	tracer.trace_simple_type::<MouseMotion>()?;
	tracer.trace_simple_type::<Widget>()?;
	// level 2
	tracer.trace_simple_type::<BroadcastEventDiscriminant>()?;
	tracer.trace_simple_type::<BroadcastMessageDiscriminant>()?;
	tracer.trace_simple_type::<DebugMessageDiscriminant>()?;
	tracer.trace_simple_type::<DialogMessageDiscriminant>()?;
	tracer.trace_simple_type::<ExportDialogMessageDiscriminant>()?;
	tracer.trace_simple_type::<FrontendMessageDiscriminant>()?;
	tracer.trace_simple_type::<InputMapperMessageDiscriminant>()?;
	tracer.trace_simple_type::<InputPreprocessorMessageDiscriminant>()?;
	tracer.trace_simple_type::<KeyDiscriminant>()?;
	tracer.trace_simple_type::<LayoutMessageDiscriminant>()?;
	tracer.trace_simple_type::<NumberInputIncrementBehavior>()?;
	tracer.trace_simple_type::<NumberInputMode>()?;
	tracer.trace_simple_type::<PivotPosition>()?;
	tracer.trace_simple_type::<PortfolioMessageDiscriminant>()?;
	tracer.trace_simple_type::<PreferencesMessageDiscriminant>()?;
	tracer.trace_simple_type::<SelectToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<SeparatorDirection>()?;
	tracer.trace_simple_type::<SeparatorType>()?;
	tracer.trace_simple_type::<ToolMessageDiscriminant>()?;
	// level 3
	tracer.trace_simple_type::<ArtboardToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<DocumentMessageDiscriminant>()?;
	tracer.trace_simple_type::<EllipseToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<EyedropperToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<FillToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<FreehandToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<GradientToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<ImaginateToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<LineToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<NavigateToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<NewDocumentDialogMessageDiscriminant>()?;
	tracer.trace_simple_type::<NodeGraphFrameToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<PathToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<PenToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<RectangleToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<ShapeToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<SplineToolMessageDiscriminant>()?;
	tracer.trace_simple_type::<TextToolMessageDiscriminant>()?;
	// level 4
	tracer.trace_simple_type::<ArtboardMessageDiscriminant>()?;
	tracer.trace_simple_type::<NavigationMessageDiscriminant>()?;
	tracer.trace_simple_type::<NodeGraphMessageDiscriminant>()?;
	tracer.trace_simple_type::<OverlaysMessageDiscriminant>()?;
	tracer.trace_simple_type::<PropertiesPanelMessageDiscriminant>()?;
	// level 5
	tracer.trace_simple_type::<TransformLayerMessageDiscriminant>()?;

	// Obtain the registry of Serde formats and serialize it in YAML (for instance).
	let registry = tracer.registry()?;
	serde_json::to_writer(std::io::stdout(), &registry)?;

	// registry
	//  to_string(&registry).unwrap();
	Ok(())
}
