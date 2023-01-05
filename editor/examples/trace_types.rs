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

use serde_reflection::{Format, Named, Registry, Tracer, TracerConfig, VariantFormat};

use graphite_editor::messages::frontend::FrontendMessage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let registry = trace_me_up()?;

	// serde_json::to_writer(std::io::stdout(), &registry)?;

	for (type_name, type_def) in &registry {
		let ts_typedef = match type_def {
			serde_reflection::ContainerFormat::UnitStruct => "{}".into(),
			serde_reflection::ContainerFormat::NewTypeStruct(inner) => format_type(inner),
			serde_reflection::ContainerFormat::TupleStruct(inner) => format_tuple(inner),
			serde_reflection::ContainerFormat::Struct(inner) => format_struct(inner),
			serde_reflection::ContainerFormat::Enum(inner) => inner.values().map(|pair| -> String { format!("{{ {}: {} }}", pair.name, format_variant_type(&pair.value)) }).collect::<Vec<String>>().join(" |\n"),
		};
		println!("export type {type_name} =\n{ts_typedef};\n");
	}
	Ok(())
}

fn format_tuple(inner: &[Format]) -> String {
	format!("[{}]", (inner.iter().map(format_type).collect::<Vec<String>>().join(", ")))
}

fn format_struct(inner: &[Named<Format>]) -> String {
	format!("{{ {} }}", (inner.iter().map(|pair| format!("{}: {}", pair.name, format_type(&pair.value))).collect::<Vec<String>>().join(",\n")))
}

fn format_type(inner: &Format) -> String {
	match inner {
		Format::Variable(_) => "any".into(),
		Format::TypeName(name) => name.clone(),
		Format::Unit => "null".into(),
		Format::Bool => "boolean".into(),
		Format::I8 => "number".into(),
		Format::I16 => "number".into(),
		Format::I32 => "number".into(),
		Format::I64 => "number".into(),
		Format::I128 => "number".into(),
		Format::U8 => "number".into(),
		Format::U16 => "number".into(),
		Format::U32 => "number".into(),
		Format::U64 => "number".into(),
		Format::U128 => "number".into(),
		Format::F32 => "number".into(),
		Format::F64 => "number".into(),
		Format::Char => "number".into(),
		Format::Str => "string".into(),
		Format::Bytes => "string".into(),
		Format::Option(inner) => format!("(undefined | {})", format_type(inner)),
		Format::Seq(inner) => format!("Array<{}>", format_type(inner)),
		Format::Map { key, value } => format!("Record<{}, {}>", format_type(key), format_type(value)),
		Format::Tuple(inner) => format_tuple(inner),
		Format::TupleArray { content, size } => format!("(Array<{}> & {{ length: {} }})", format_type(content), size),
	}
}

fn format_variant_type(format: &VariantFormat) -> String {
	match format {
		VariantFormat::Variable(_) => "any".into(),
		VariantFormat::Unit => "null".into(),
		VariantFormat::NewType(inner) => format_type(inner),
		VariantFormat::Tuple(inner) => format_tuple(inner),
		VariantFormat::Struct(inner) => format_struct(inner),
	}
}

fn trace_me_up() -> Result<Registry, Box<dyn std::error::Error>> {
	let mut tracer = Tracer::new(TracerConfig::default());
	tracer.trace_simple_type::<FrontendMessage>()?;
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
	tracer.trace_simple_type::<ArtboardMessageDiscriminant>()?;
	tracer.trace_simple_type::<NavigationMessageDiscriminant>()?;
	tracer.trace_simple_type::<NodeGraphMessageDiscriminant>()?;
	tracer.trace_simple_type::<OverlaysMessageDiscriminant>()?;
	tracer.trace_simple_type::<PropertiesPanelMessageDiscriminant>()?;
	tracer.trace_simple_type::<TransformLayerMessageDiscriminant>()?;
	let registry = tracer.registry()?;
	Ok(registry)
}
