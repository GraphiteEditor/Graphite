use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, LayoutTarget};
use crate::messages::portfolio::document::data_panel::{DataPanelMessage, PathStep};
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shapes::shape_utility::format_rounded;
use crate::messages::tool::tool_messages::tool_prelude::*;
use crate::node_graph_executor::InspectedContext;
use glam::{Affine2, DAffine2, Vec2};
use graph_craft::document::NodeId;
use graphene_std::animation::RealTimeMode;
use graphene_std::blending::BlendMode;
use graphene_std::color::SRGBA8;
use graphene_std::core_types::misc::format_f64;
use graphene_std::gradient::Gradient;
use graphene_std::list::{INTERNAL_ATTRIBUTE_PREFIX, Item, List, NodeIdPath};
use graphene_std::math::float_noise::round_away_float_noise;
use graphene_std::raster::{
	AdjustmentChannel, CellularDistanceFunction, CellularReturnType, DesaturateMethod, DomainWarpType, FractalType, HueSaturationRange, NoiseType, RedGreenBlue, RelativeAbsolute,
	SelectiveColorChoice, TonalRange,
};
use graphene_std::raster_types::{CPU, GPU, Raster};
use graphene_std::text::TextAlign;
use graphene_std::text_nodes::{StringCapitalization, TextDenomination};
use graphene_std::transfer_curve::TransferCurve;
use graphene_std::transform::Footprint;
use graphene_std::transform::{ReferencePoint, ScaleType};
use graphene_std::vector::misc::{
	ArcType, BooleanOperation, BoxCorners, CentroidType, ExtrudeJoiningAlgorithm, GridTopology, GridType, InterpolationDistribution, PointSpacingType, RowsOrColumns, SpiralType, UnweldMethod,
	WeldMethod,
};
use graphene_std::vector::style::{
	DashPattern, FillChoice, GradientForm, GradientHueDirection, GradientInterpolation, GradientRamp, GradientSettings, GradientSpace, GradientSpread, StrokeAlign, StrokeCap, StrokeJoin,
};
use graphene_std::vector::{CurveType, Path, PointId, QRCodeErrorCorrectionLevel, SegmentId, Vector};
use graphene_std::{Appearance, Artboard, Color, ContextFeatures, ContextOverrides, Cover, Coverage, Effect, EffectKind, Effects, GlowSource, Graphic};
use kurbo::BezPath;
use std::any::Any;
use std::sync::Arc;

#[derive(ExtractField)]
pub struct DataPanelMessageContext<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
	pub data_panel_open: bool,
}

/// The Data panel allows for graph data to be previewed.
#[derive(Default, Debug, Clone, ExtractField)]
pub struct DataPanelMessageHandler {
	/// Full path from the root network to the introspected node, with the node itself as the last element.
	/// Empty when nothing is being introspected.
	introspected_node_path: Vec<NodeId>,
	introspected_data: Option<Arc<dyn Any + Send + Sync>>,
	context: InspectedContext,
	element_path: Vec<PathStep>,
}

#[message_handler_data]
impl MessageHandler<DataPanelMessage, DataPanelMessageContext<'_>> for DataPanelMessageHandler {
	fn process_message(&mut self, message: DataPanelMessage, responses: &mut VecDeque<Message>, context: DataPanelMessageContext) {
		match message {
			DataPanelMessage::UpdateLayout { mut inspect_result } => {
				self.introspected_data = inspect_result.take_data();
				self.introspected_node_path = inspect_result.inspect_node_path;
				self.context = inspect_result.context;
				self.update_layout(responses, context);
			}
			DataPanelMessage::ClearLayout => {
				self.introspected_node_path.clear();
				self.introspected_data = None;
				self.context = InspectedContext::default();
				self.element_path.clear();
				self.update_layout(responses, context);
			}
			DataPanelMessage::Refresh => {
				// Re-render against the current network_interface without disturbing introspected_data or the breadcrumb path.
				// Always re-renders, even when introspected_data is None, since the header still shows the inspected node's
				// name/lock/visibility state from the network interface and that state can change independently of the data.
				self.update_layout(responses, context);
			}

			DataPanelMessage::PushToElementPath { step } => {
				self.element_path.push(step);
				self.update_layout(responses, context);
			}
			DataPanelMessage::TruncateElementPath { len } => {
				self.element_path.truncate(len);
				self.update_layout(responses, context);
			}
			DataPanelMessage::SetContextOverrides { overrides } => {
				responses.add(PortfolioMessage::SubmitInspectContextOverrides { overrides });
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(DataPanelMessage;)
	}
}

impl DataPanelMessageHandler {
	fn update_layout(&mut self, responses: &mut VecDeque<Message>, context: DataPanelMessageContext<'_>) {
		let DataPanelMessageContext {
			network_interface,
			selection_network_path,
			..
		} = context;

		let mut layout_data = LayoutData {
			current_depth: 0,
			desired_path: &mut self.element_path,
			network_interface: &*network_interface,
			selection_network_path,
			node_lookup_network_path: Vec::new(),
			gradient_settings: GradientSettings::default(),
			breadcrumbs: Vec::new(),
		};

		// Main data visualization
		let layout = Layout(
			self.introspected_data
				.as_ref()
				.map(|instrospected_data| generate_layout(instrospected_data, &mut layout_data).unwrap_or_else(|| label("Visualization of this data type is not yet supported")))
				.unwrap_or_default(),
		);

		let mut widgets = Vec::new();

		// The selected layer or node, by its icon and name
		if let Some((node_id, parent_path)) = self.introspected_node_path.split_last() {
			widgets.extend([
				IconLabel::new(network_interface.node_icon(node_id, parent_path)).widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				TextLabel::new(network_interface.display_name(node_id, parent_path)).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			]);
		}

		// Element path breadcrumbs
		if !layout_data.breadcrumbs.is_empty() {
			let labels = layout_data.breadcrumbs.iter().map(|label| truncate_breadcrumb_label(label)).collect();
			let breadcrumb = BreadcrumbTrailButtons::new(labels)
				.on_update(|&len| DataPanelMessage::TruncateElementPath { len: len as usize }.into())
				.widget_instance();
			widgets.push(breadcrumb);
		}

		let path_layout = Layout(if widgets.is_empty() { Vec::new() } else { vec![LayoutGroup::row(widgets)] });

		let context_widgets = context_widgets(&self.context);
		let context_layout = Layout(if context_widgets.is_empty() { Vec::new() } else { vec![LayoutGroup::row(context_widgets)] });

		responses.add(LayoutMessage::SendLayout {
			layout: path_layout,
			layout_target: LayoutTarget::DataPanelPath,
		});
		responses.add(LayoutMessage::SendLayout {
			layout: context_layout,
			layout_target: LayoutTarget::DataPanelContext,
		});
		responses.add(LayoutMessage::SendLayout {
			layout,
			layout_target: LayoutTarget::DataPanel,
		});
	}
}

/// The inspected node's context, as inputs for overriding the features its output depends on.
fn context_widgets(context: &InspectedContext) -> Vec<WidgetInstance> {
	let InspectedContext {
		features,
		positions,
		variables,
		overrides,
		dependencies,
	} = context;
	let current = EditableContext::new(features, overrides);
	let label = |name: &str, overridden: bool| TextLabel::new(name).bold(true).italic(overridden).widget_instance();
	let mut groups: Vec<Vec<WidgetInstance>> = Vec::new();

	if dependencies.contains(ContextFeatures::FOOTPRINT) {
		let (scale, angle, translation) = current.footprint.transform.to_scale_angle_translation();
		let resolution = current.footprint.resolution;
		groups.push(vec![
			label("Footprint", overrides.footprint.is_some()),
			current
				.input(translation.x, |context, x| edit_transform(context, |_, _, translation| translation.x = x))
				.label("X")
				.widget_instance(),
			current
				.input(translation.y, |context, y| edit_transform(context, |_, _, translation| translation.y = y))
				.label("Y")
				.widget_instance(),
			current
				.input(scale.x, |context, value| edit_transform(context, |scale, _, _| *scale = DVec2::splat(value)))
				.label("Scale")
				.widget_instance(),
			current
				.input(angle.to_degrees(), |context, degrees| edit_transform(context, |_, angle, _| *angle = degrees.to_radians()))
				.label("Angle")
				.unit("°")
				.widget_instance(),
			current
				.input(resolution.x as f64, |context, width| context.footprint.resolution.x = width.max(0.) as u32)
				.label("W")
				.int()
				.unit(" px")
				.widget_instance(),
			current
				.input(resolution.y as f64, |context, height| context.footprint.resolution.y = height.max(0.) as u32)
				.label("H")
				.int()
				.unit(" px")
				.widget_instance(),
		]);
	}

	if dependencies.contains(ContextFeatures::REAL_TIME) {
		let time_input = current.input(current.real_time, |context, time| context.real_time = time).unit(" s");
		groups.push(vec![label("Real Time", overrides.real_time.is_some()), time_input.widget_instance()]);
	}

	if dependencies.contains(ContextFeatures::ANIMATION_TIME) {
		let time_input = current.input(current.animation_time, |context, time| context.animation_time = time).unit(" s");
		groups.push(vec![label("Animation Time", overrides.animation_time.is_some()), time_input.widget_instance()]);
	}

	if dependencies.contains(ContextFeatures::POINTER_POSITION) {
		let pointer = current.pointer_position;
		groups.push(vec![
			label("Pointer", overrides.pointer_position.is_some()),
			current.input(pointer.x, |context, x| context.pointer_position.x = x).label("X").widget_instance(),
			current.input(pointer.y, |context, y| context.pointer_position.y = y).label("Y").widget_instance(),
		]);
	}

	if dependencies.contains(ContextFeatures::INDEX) {
		let mut group = vec![label("Index", overrides.index.is_some())];
		for (level, &value) in current.index.iter().enumerate() {
			let level_input = current.input(value as f64, move |context, value| {
				if let Some(index) = context.index.get_mut(level) {
					*index = value.max(0.) as usize;
				}
			});
			group.push(level_input.int().min(0.).sizing(WidgetSizing::fixed(48)).widget_instance());
		}
		groups.push(group);
	}

	if dependencies.contains(ContextFeatures::POSITION) && !positions.is_empty() {
		let positions = positions
			.iter()
			.map(|position| format!("({}, {})", format_f64(position.x), format_f64(position.y)))
			.collect::<Vec<_>>()
			.join(", ");
		groups.push(vec![label("Position", false), TextLabel::new(positions).widget_instance()]);
	}

	if dependencies.contains(ContextFeatures::VARARGS) && *variables > 0 {
		groups.push(vec![label("Variables", false), TextLabel::new(variables.to_string()).widget_instance()]);
	}

	if !overrides.is_empty() {
		groups.push(vec![
			TextButton::new("Reset")
				.icon("Reset")
				.tooltip_description("Evaluate the node with the context it was evaluated with in the graph.")
				.on_update(|_| {
					DataPanelMessage::SetContextOverrides {
						overrides: ContextOverrides::default(),
					}
					.into()
				})
				.widget_instance(),
		]);
	}

	let mut widgets = Vec::new();
	for (i, group) in groups.into_iter().enumerate() {
		if i > 0 {
			widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		}
		for (j, widget) in group.into_iter().enumerate() {
			if j > 0 {
				widgets.push(Separator::new(SeparatorStyle::Related).widget_instance());
			}
			widgets.push(widget);
		}
	}
	widgets
}

/// The features of a context the Data panel can edit, with defaults for those it lacks.
#[derive(Clone, PartialEq)]
struct EditableContext {
	footprint: Footprint,
	real_time: f64,
	animation_time: f64,
	pointer_position: DVec2,
	index: Vec<usize>,
	overrides: ContextOverrides,
}

impl EditableContext {
	fn new(features: &ContextOverrides, overrides: &ContextOverrides) -> Self {
		Self {
			footprint: features.footprint.unwrap_or(Footprint::DEFAULT),
			real_time: features.real_time.unwrap_or_default(),
			animation_time: features.animation_time.unwrap_or_default(),
			pointer_position: features.pointer_position.unwrap_or_default(),
			index: features.index.clone().filter(|index| !index.is_empty()).unwrap_or_else(|| vec![0]),
			overrides: overrides.clone(),
		}
	}

	/// An input that overrides the features its edit changes, on top of the existing overrides.
	fn input(&self, value: f64, edit: impl Fn(&mut Self, f64) + Send + Sync + 'static) -> NumberInput {
		let current = self.clone();

		NumberInput::new(Some(value))
			.increment_behavior(NumberInputIncrementBehavior::None)
			.sizing(WidgetSizing::fixed(88))
			.on_update(move |number_input: &NumberInput| {
				let Some(value) = number_input.value else { return Message::NoOp };
				let mut edited = current.clone();
				edit(&mut edited, value);
				DataPanelMessage::SetContextOverrides {
					overrides: current.overrides_after_edit(edited),
				}
				.into()
			})
	}

	fn overrides_after_edit(&self, edited: Self) -> ContextOverrides {
		let overrides = &self.overrides;
		ContextOverrides {
			footprint: (overrides.footprint.is_some() || edited.footprint != self.footprint).then_some(edited.footprint),
			real_time: (overrides.real_time.is_some() || edited.real_time != self.real_time).then_some(edited.real_time),
			animation_time: (overrides.animation_time.is_some() || edited.animation_time != self.animation_time).then_some(edited.animation_time),
			pointer_position: (overrides.pointer_position.is_some() || edited.pointer_position != self.pointer_position).then_some(edited.pointer_position),
			index: (overrides.index.is_some() || edited.index != self.index).then_some(edited.index),
		}
	}
}

fn edit_transform(context: &mut EditableContext, edit: impl FnOnce(&mut DVec2, &mut f64, &mut DVec2)) {
	let (mut scale, mut angle, mut translation) = context.footprint.transform.to_scale_angle_translation();
	edit(&mut scale, &mut angle, &mut translation);
	context.footprint.transform = DAffine2::from_scale_angle_translation(scale, angle, translation);
}

struct LayoutData<'a> {
	current_depth: usize,
	desired_path: &'a mut Vec<PathStep>,
	network_interface: &'a NodeNetworkInterface,
	/// The network open in the graph, which is where selecting a node takes effect.
	selection_network_path: &'a [NodeId],
	/// The `network_path` to use when resolving a `NodeId` against the network interface.
	/// Defaults to root (`&[]`); `List<NodeId>` rendering temporarily sets it to the path's prefix so nested
	/// layers (e.g. inside a Ctrl+M-merged custom subgraph) resolve correctly.
	node_lookup_network_path: Vec<NodeId>,
	/// The whole-ramp settings to preview a `Gradient` element with, since they live in the attributes beside it
	/// rather than in the stop list itself. The enclosing `Item`/`List` sets it to the owning row's attributes.
	gradient_settings: GradientSettings,
	breadcrumbs: Vec<String>,
}

/// Calls the macro with its arguments followed by the list of every value type the Data panel can show.
macro_rules! inspectable_types {
	($callback:ident $(, $argument:expr)*) => {
		$callback!($($argument,)* [
			List<Artboard>,
			List<Graphic>,
			List<Vector>,
			List<Raster<CPU>>,
			List<Raster<GPU>>,
			List<Color>,
			List<Gradient>,
			List<String>,
			List<f64>,
			List<i64>,
			List<bool>,
			List<DVec2>,
			List<DAffine2>,
			List<BlendMode>,
			List<GradientForm>,
			List<GradientSpread>,
			List<GradientSpace>,
			List<GradientHueDirection>,
			List<GradientInterpolation>,
			List<DashPattern>,
			List<BoxCorners>,
			List<TransferCurve>,
			List<StrokeJoin>,
			List<StrokeAlign>,
			List<StrokeCap>,
			List<WeldMethod>,
			List<UnweldMethod>,
			List<ExtrudeJoiningAlgorithm>,
			List<PointSpacingType>,
			List<StringCapitalization>,
			List<TextDenomination>,
			List<DesaturateMethod>,
			List<RedGreenBlue>,
			List<RelativeAbsolute>,
			List<SelectiveColorChoice>,
			List<TonalRange>,
			List<AdjustmentChannel>,
			List<HueSaturationRange>,
			List<ScaleType>,
			List<ReferencePoint>,
			List<CentroidType>,
			List<BooleanOperation>,
			List<NoiseType>,
			List<FractalType>,
			List<CellularDistanceFunction>,
			List<CellularReturnType>,
			List<DomainWarpType>,
			List<RealTimeMode>,
			List<GridType>,
			List<GridTopology>,
			List<ArcType>,
			List<SpiralType>,
			List<TextAlign>,
			List<QRCodeErrorCorrectionLevel>,
			List<InterpolationDistribution>,
			List<RowsOrColumns>,
			Item<Artboard>,
			Item<Graphic>,
			Item<Vector>,
			Item<Raster<CPU>>,
			Item<Raster<GPU>>,
			Item<Color>,
			Item<Gradient>,
			Item<String>,
			Item<f64>,
			Item<i64>,
			Item<bool>,
			Item<DVec2>,
			Item<DAffine2>,
			Item<BlendMode>,
			Item<GradientForm>,
			Item<GradientSpread>,
			Item<GradientSpace>,
			Item<GradientHueDirection>,
			Item<GradientInterpolation>,
			Item<DashPattern>,
			Item<BoxCorners>,
			Item<TransferCurve>,
			Item<StrokeJoin>,
			Item<StrokeAlign>,
			Item<StrokeCap>,
			Item<WeldMethod>,
			Item<UnweldMethod>,
			Item<ExtrudeJoiningAlgorithm>,
			Item<PointSpacingType>,
			Item<StringCapitalization>,
			Item<TextDenomination>,
			Item<DesaturateMethod>,
			Item<RedGreenBlue>,
			Item<RelativeAbsolute>,
			Item<SelectiveColorChoice>,
			Item<TonalRange>,
			Item<AdjustmentChannel>,
			Item<HueSaturationRange>,
			Item<ScaleType>,
			Item<ReferencePoint>,
			Item<CentroidType>,
			Item<BooleanOperation>,
			Item<NoiseType>,
			Item<FractalType>,
			Item<CellularDistanceFunction>,
			Item<CellularReturnType>,
			Item<DomainWarpType>,
			Item<RealTimeMode>,
			Item<GridType>,
			Item<GridTopology>,
			Item<ArcType>,
			Item<SpiralType>,
			Item<TextAlign>,
			Item<QRCodeErrorCorrectionLevel>,
			Item<InterpolationDistribution>,
			Item<RowsOrColumns>,
		])
	};
}

macro_rules! generate_layout_downcast {
	($introspected_data:expr, $data:expr, [ $($ty:ty),* $(,)? ]) => {
		if false { None }
		$(
			else if let Some(value) = $introspected_data.downcast_ref::<$ty>() {
				Some(value.layout_with_breadcrumb($data))
			}
		)*
		else { None }
	}
}

macro_rules! shareable_downcast {
	($output:expr, [ $($ty:ty),* $(,)? ]) => {{
		let output = $output;
		$(
			let output = match output.downcast::<$ty>() {
				Ok(value) => return Some(Arc::new(*value)),
				Err(output) => output,
			};
		)*
		drop(output);
		None
	}};
}

/// The type-erased output of an evaluated node as a shareable value, if the Data panel can show its type.
pub fn shareable_inspect_output(output: graphene_std::core_types::registry::Any) -> Option<Arc<dyn Any + Send + Sync + 'static>> {
	let output = match output.downcast::<Item<NodeIdPath>>() {
		Ok(path) => return Some(Arc::new(*path)),
		Err(output) => output,
	};
	inspectable_types!(shareable_downcast, output)
}
// TODO: We simply try all these types sequentially. Find a better strategy.
fn generate_layout(introspected_data: &Arc<dyn std::any::Any + Send + Sync + 'static>, data: &mut LayoutData) -> Option<Vec<LayoutGroup>> {
	// `Item<NodeIdPath>` is interpreted as a path (e.g. the value produced by `path_of_subgraph`), shown as a
	// `List` where each item's NodeId resolves against the prefix made up of the items above it.
	if let Some(path) = introspected_data.downcast_ref::<Item<NodeIdPath>>() {
		return Some(table_node_id_path_layout_with_breadcrumb(&path.element().0, data));
	}
	inspectable_types!(generate_layout_downcast, introspected_data, data)
}

fn column_headings(value: &[&str]) -> Vec<WidgetInstance> {
	value.iter().map(|text| TextLabel::new(*text).widget_instance()).collect()
}

fn single_widget_cells(widgets: Vec<WidgetInstance>) -> Vec<Vec<WidgetInstance>> {
	widgets.into_iter().map(|widget| vec![widget]).collect()
}

fn label(x: impl Into<String>) -> Vec<LayoutGroup> {
	let error = vec![TextLabel::new(x).widget_instance()];
	vec![LayoutGroup::row(error)]
}

trait TableItemLayout {
	fn type_name() -> &'static str;
	fn identifier(&self) -> String;
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		data.breadcrumbs.push(self.identifier());
		self.value_page(data)
	}
	/// Renders this value as the inline widgets filling one table cell inside an item of a `List`.
	/// `target` is the [`PathStep`] to push when a widget is clicked to drill into the value.
	/// `data` provides shared context (notably `network_interface`) for types whose label or content
	/// depends on lookup beyond their own value (e.g. `NodeId` resolving a node's display name).
	/// The default is a single button labeled with `identifier()`. Types whose values are best shown
	/// inline (colors, transforms, primitives, etc.) override this to ignore `target` and return a
	/// richer non-navigating widget, optionally joined by companions like a drill-in button.
	fn value_widgets(&self, target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![
			TextButton::new(self.identifier())
				.on_update(move |_| DataPanelMessage::PushToElementPath { step: target.clone() }.into())
				.narrow(true)
				.widget_instance(),
		]
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![]
	}
}

impl<T: TableItemLayout> TableItemLayout for Item<T> {
	fn type_name() -> &'static str {
		T::type_name()
	}
	fn identifier(&self) -> String {
		self.element().identifier()
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		if let Some(step) = data.desired_path.get(data.current_depth).cloned() {
			match step {
				PathStep::Element(_) => {
					data.current_depth += 1;
					let result = self.element().layout_with_breadcrumb(data);
					data.current_depth -= 1;
					return result;
				}
				PathStep::Attribute { key, .. } => {
					if let Some(any) = self.attributes().get_any(&key) {
						data.current_depth += 1;
						if let Some(result) = drilldown_attribute_layout(any, data) {
							data.current_depth -= 1;
							return result;
						}
						data.current_depth -= 1;
						warn!("Drilldown unsupported for attribute {key:?}");
					}
					data.desired_path.truncate(data.current_depth);
				}
			}
		}

		let attribute_keys: Vec<String> = self.attributes().keys().map(str::to_string).collect();

		// A single element, so no leading ID column, unlike the `List` table
		let saved_gradient_settings = data.gradient_settings;
		data.gradient_settings = GradientSettings::from_item_attributes(self);
		let mut values = vec![self.element().value_widgets(PathStep::Element(0), data)];
		data.gradient_settings = saved_gradient_settings;
		for key in &attribute_keys {
			let target = PathStep::Attribute { row: 0, key: key.clone() };
			let cell = self.attributes().get_any(key).and_then(|any| dispatch_value_widgets(any, target, data)).unwrap_or_else(|| {
				let text = self.attributes().display_value(key, display_value_override).unwrap_or_else(|| "-".to_string());
				vec![text_preview_label(text)]
			});
			values.push(cell);
		}

		let mut column_names = vec!["element"];
		column_names.extend(attribute_keys.iter().map(|s| s.as_str()));

		vec![LayoutGroup::table_of_cells(vec![single_widget_cells(column_headings(&column_names)), values], false)]
	}
}

impl<T: TableItemLayout> TableItemLayout for List<T> {
	fn type_name() -> &'static str {
		"List"
	}
	fn identifier(&self) -> String {
		format!("{}[] ({} item{})", T::type_name(), self.len(), if self.len() == 1 { "" } else { "s" })
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		if let Some(step) = data.desired_path.get(data.current_depth).cloned() {
			match step {
				PathStep::Element(index) => {
					if let Some(element) = self.element(index) {
						data.current_depth += 1;
						let result = element.layout_with_breadcrumb(data);
						data.current_depth -= 1;
						return result;
					} else {
						warn!("Desired path truncated");
						data.desired_path.truncate(data.current_depth);
					}
				}
				PathStep::Attribute { row, key } => {
					if let Some(any) = self.attribute_any(&key, row) {
						data.current_depth += 1;
						if let Some(result) = drilldown_attribute_layout(any, data) {
							data.current_depth -= 1;
							return result;
						}
						data.current_depth -= 1;
						warn!("Drilldown unsupported for attribute {key:?}");
					}
					data.desired_path.truncate(data.current_depth);
				}
			}
		}

		let attribute_keys: Vec<String> = self.attribute_keys().filter(|key| !key.starts_with(INTERNAL_ATTRIBUTE_PREFIX)).map(str::to_string).collect();

		let mut rows = (0..self.len())
			.map(|index| {
				let element = self.element(index).unwrap();
				let saved_gradient_settings = data.gradient_settings;
				data.gradient_settings = GradientSettings::from_list_row_attributes(self, index);
				let mut values = vec![
					vec![TextLabel::new(format!("{index}")).narrow(true).widget_instance()],
					element.value_widgets(PathStep::Element(index), data),
				];
				data.gradient_settings = saved_gradient_settings;
				for key in &attribute_keys {
					let target = PathStep::Attribute { row: index, key: key.clone() };
					let cell = self.attribute_any(key, index).and_then(|any| dispatch_value_widgets(any, target, data)).unwrap_or_else(|| {
						let text = self.attribute_display_value(key, index, display_value_override).unwrap_or_else(|| "-".to_string());
						vec![text_preview_label(text)]
					});
					values.push(cell);
				}
				values
			})
			.collect::<Vec<_>>();

		let mut column_names = vec!["", "element"];
		column_names.extend(attribute_keys.iter().map(|s| s.as_str()));
		rows.insert(0, single_widget_cells(column_headings(&column_names)));

		vec![LayoutGroup::table_of_cells(rows, false)]
	}
}

impl TableItemLayout for Artboard {
	fn type_name() -> &'static str {
		"Artboard"
	}
	fn identifier(&self) -> String {
		self.as_graphic_list().identifier()
	}
	// Don't put a breadcrumb for Artboard
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.as_graphic_list().layout_with_breadcrumb(data)
	}
}

impl TableItemLayout for graphene_std::brush::Stroke {
	fn type_name() -> &'static str {
		"Stroke"
	}
	fn identifier(&self) -> String {
		format!("Stroke ({} {})", self.len(), if self.len() == 1 { "sample" } else { "samples" })
	}
}

impl TableItemLayout for DashPattern {
	fn type_name() -> &'static str {
		"DashPattern"
	}
	fn identifier(&self) -> String {
		"DashPattern".to_string()
	}
	// The wrapping `Item` already contributes the breadcrumb; the inner list supplies the next level
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	// Label the spreadsheet's element button with the inner list's identifier, like Artboard
	fn value_widgets(&self, target: PathStep, data: &LayoutData) -> Vec<WidgetInstance> {
		self.0.value_widgets(target, data)
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.0.layout_with_breadcrumb(data)
	}
}

impl TableItemLayout for Appearance {
	fn type_name() -> &'static str {
		"Appearance"
	}
	fn identifier(&self) -> String {
		"Appearance".to_string()
	}
	// The wrapping `Item` already contributes the breadcrumb; the inner list supplies the next level
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	// Label the spreadsheet's element button with the inner list's identifier, like Artboard
	fn value_widgets(&self, target: PathStep, data: &LayoutData) -> Vec<WidgetInstance> {
		self.0.value_widgets(target, data)
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.0.layout_with_breadcrumb(data)
	}
}

impl TableItemLayout for Coverage {
	fn type_name() -> &'static str {
		"Coverage"
	}
	fn identifier(&self) -> String {
		"Coverage".to_string()
	}
	// The wrapping row already contributes the breadcrumb; the inner item supplies the next level
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	fn value_widgets(&self, target: PathStep, data: &LayoutData) -> Vec<WidgetInstance> {
		self.0.value_widgets(target, data)
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.0.layout_with_breadcrumb(data)
	}
}

impl TableItemLayout for Effects {
	fn type_name() -> &'static str {
		"Effects"
	}
	fn identifier(&self) -> String {
		"Effects".to_string()
	}
	// The wrapping `Item` already contributes the breadcrumb; the inner list supplies the next level
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	// Label the spreadsheet's element button with the inner list's identifier, like Appearance
	fn value_widgets(&self, target: PathStep, data: &LayoutData) -> Vec<WidgetInstance> {
		self.0.value_widgets(target, data)
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.0.layout_with_breadcrumb(data)
	}
}

impl TableItemLayout for Effect {
	fn type_name() -> &'static str {
		"Effect"
	}
	fn identifier(&self) -> String {
		"Effect".to_string()
	}
	// The wrapping row already contributes the breadcrumb; the inner item supplies the next level
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	fn value_widgets(&self, target: PathStep, data: &LayoutData) -> Vec<WidgetInstance> {
		self.0.value_widgets(target, data)
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.0.layout_with_breadcrumb(data)
	}
}

impl TableItemLayout for TransferCurve {
	fn type_name() -> &'static str {
		"TransferCurve"
	}
	fn identifier(&self) -> String {
		let points = self.points().len();
		format!("Transfer Curve ({points} {})", if points == 1 { "point" } else { "points" })
	}
	// The wrapping `Item` already contributes the breadcrumb; the inner list supplies the next level
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	fn value_widgets(&self, target: PathStep, data: &LayoutData) -> Vec<WidgetInstance> {
		self.0.value_widgets(target, data)
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.0.layout_with_breadcrumb(data)
	}
}

impl TableItemLayout for BoxCorners {
	fn type_name() -> &'static str {
		"BoxCorners"
	}
	fn identifier(&self) -> String {
		"BoxCorners".to_string()
	}
	// The wrapping `Item` already contributes the breadcrumb; the inner list supplies the next level
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	// Label the spreadsheet's element button with the inner list's identifier, like Artboard
	fn value_widgets(&self, target: PathStep, data: &LayoutData) -> Vec<WidgetInstance> {
		self.0.value_widgets(target, data)
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.0.layout_with_breadcrumb(data)
	}
}

impl TableItemLayout for graphene_std::core_types::none::None {
	fn type_name() -> &'static str {
		"None"
	}
	fn identifier(&self) -> String {
		"None".to_string()
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		label("None")
	}
}

impl TableItemLayout for Graphic {
	fn type_name() -> &'static str {
		"Graphic"
	}
	fn identifier(&self) -> String {
		match self {
			Self::None(item) => item.identifier(),
			Self::Graphic(item) => item.identifier(),
			Self::Vector(item) => item.identifier(),
			Self::RasterCPU(item) => item.identifier(),
			Self::RasterGPU(item) => item.identifier(),
			Self::Color(item) => item.identifier(),
			Self::Gradient(item) => item.identifier(),
			Self::Text(item) => item.identifier(),
			Self::NoneList(list) => list.identifier(),
			Self::GraphicList(list) => list.identifier(),
			Self::VectorList(list) => list.identifier(),
			Self::RasterCPUList(list) => list.identifier(),
			Self::RasterGPUList(list) => list.identifier(),
			Self::ColorList(list) => list.identifier(),
			Self::GradientList(list) => list.identifier(),
			Self::TextList(list) => list.identifier(),
			Self::StrokeList(list) => list.identifier(),
		}
	}
	// Don't put a breadcrumb for Graphic
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		match self {
			Self::None(item) => item.layout_with_breadcrumb(data),
			Self::Graphic(item) => item.layout_with_breadcrumb(data),
			Self::Vector(item) => item.layout_with_breadcrumb(data),
			Self::RasterCPU(item) => item.layout_with_breadcrumb(data),
			Self::RasterGPU(item) => item.layout_with_breadcrumb(data),
			Self::Color(item) => item.layout_with_breadcrumb(data),
			Self::Gradient(item) => item.layout_with_breadcrumb(data),
			Self::Text(item) => item.layout_with_breadcrumb(data),
			Self::NoneList(list) => list.layout_with_breadcrumb(data),
			Self::GraphicList(list) => list.layout_with_breadcrumb(data),
			Self::VectorList(list) => list.layout_with_breadcrumb(data),
			Self::RasterCPUList(list) => list.layout_with_breadcrumb(data),
			Self::RasterGPUList(list) => list.layout_with_breadcrumb(data),
			Self::ColorList(list) => list.layout_with_breadcrumb(data),
			Self::GradientList(list) => list.layout_with_breadcrumb(data),
			Self::TextList(list) => list.layout_with_breadcrumb(data),
			Self::StrokeList(list) => list.layout_with_breadcrumb(data),
		}
	}
}

// The pool is a plain point list with its IDs riding its attributes, so the generic list layout shows it as-is
impl TableItemLayout for Vector {
	fn type_name() -> &'static str {
		"Vector"
	}
	fn identifier(&self) -> String {
		self.0.identifier()
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.0.value_page(data)
	}
}

// A path is its list of point references, with segment IDs and smooth flags riding its attributes
impl TableItemLayout for Path {
	fn type_name() -> &'static str {
		"Path"
	}
	fn identifier(&self) -> String {
		self.0.identifier()
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.0.value_page(data)
	}
}

// Only the click target attribute holds a bezpath, so it's named for what it does
impl TableItemLayout for BezPath {
	fn type_name() -> &'static str {
		"Click Target"
	}
	fn identifier(&self) -> String {
		Self::type_name().to_string()
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![TextLabel::new(self.identifier()).narrow(true).widget_instance()]
	}
}

impl TableItemLayout for Raster<CPU> {
	fn type_name() -> &'static str {
		"Raster"
	}
	fn identifier(&self) -> String {
		format!("Raster ({} x {})", self.width, self.height)
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let raster = self.data();

		if raster.width == 0 || raster.height == 0 {
			let widgets = vec![TextLabel::new("Image has no area").widget_instance()];
			return vec![LayoutGroup::row(widgets)];
		}

		let widgets = vec![ImageLabel::new(self.png_data_url().to_string()).widget_instance()];
		vec![LayoutGroup::row(widgets)]
	}
}

impl TableItemLayout for Raster<GPU> {
	fn type_name() -> &'static str {
		"Raster"
	}
	fn identifier(&self) -> String {
		format!("Raster ({} x {})", self.data().width(), self.data().height())
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new("This raster data is a texture on the GPU. It currently cannot be displayed here.").widget_instance()];
		vec![LayoutGroup::row(widgets)]
	}
}

impl TableItemLayout for Color {
	fn type_name() -> &'static str {
		"Color"
	}
	fn identifier(&self) -> String {
		format!("Color (#{})", SRGBA8::from(*self).to_rgba_hex())
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![
			ColorInput::new(FillChoice::<SRGBA8>::from(&FillChoice::Solid(*self)))
				.disabled(true)
				.menu_direction(Some(MenuDirection::Top))
				.narrow(true)
				.widget_instance(),
		]
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(self.value_widgets(PathStep::Element(0), _data))]
	}
}

impl TableItemLayout for Gradient {
	fn type_name() -> &'static str {
		"Gradient"
	}
	fn identifier(&self) -> String {
		format!("Gradient ({} stops)", self.len())
	}
	// The wrapping `Item` already contributes the breadcrumb; the inner list supplies the next level
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.value_page(data)
	}
	// The preview widget doesn't navigate, so a drill-in button beside it opens the newtype's underlying color list
	fn value_widgets(&self, target: PathStep, data: &LayoutData) -> Vec<WidgetInstance> {
		vec![
			TextButton::new(self.as_color_list().identifier())
				.on_update(move |_| DataPanelMessage::PushToElementPath { step: target.clone() }.into())
				.narrow(true)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			ColorInput::new(FillChoice::<SRGBA8>::Gradient(GradientRamp::from(self).with_settings(data.gradient_settings)))
				.menu_direction(Some(MenuDirection::Top))
				.disabled(true)
				.narrow(true)
				.widget_instance(),
		]
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.as_color_list().layout_with_breadcrumb(data)
	}
}

impl TableItemLayout for i64 {
	fn type_name() -> &'static str {
		"Integer"
	}
	fn identifier(&self) -> String {
		format!("{self}")
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![TextLabel::new(self.identifier()).selectable(true).narrow(true).widget_instance()]
	}
}

// Denoised so 0.1 + 0.2 reads as 0.3 rather than 0.30000000000000004. We don't do this for f32 because it lacks precision to reliably distinguish between intentional digits and noise.
impl TableItemLayout for f64 {
	fn type_name() -> &'static str {
		"Number"
	}
	fn identifier(&self) -> String {
		format_f64(round_away_float_noise(*self))
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![TextLabel::new(self.identifier()).selectable(true).narrow(true).widget_instance()]
	}
}

impl TableItemLayout for u8 {
	fn type_name() -> &'static str {
		"Byte"
	}
	fn identifier(&self) -> String {
		format!("0x{self:02X} ({self})")
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![TextLabel::new(self.identifier()).selectable(true).narrow(true).widget_instance()]
	}
}

impl TableItemLayout for bool {
	fn type_name() -> &'static str {
		"Bool"
	}
	fn identifier(&self) -> String {
		"Bool".to_string()
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![CheckboxInput::new(*self).disabled(true).widget_instance()]
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(self.value_widgets(PathStep::Element(0), _data))]
	}
}

impl TableItemLayout for String {
	fn type_name() -> &'static str {
		"String"
	}
	fn identifier(&self) -> String {
		format!("\"{}\"", string_preview(self))
	}
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		data.breadcrumbs.push(character_count_label(self));
		self.value_page(data)
	}
	// The preview truncates for length and line breaks, so a button beside it reaches the full text, labeled by length
	fn value_widgets(&self, target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![
			TextLabel::new(string_preview(self)).enquote(true).selectable(true).monospace(true).narrow(true).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextButton::new(character_count_label(self))
				.on_update(move |_| DataPanelMessage::PushToElementPath { step: target.clone() }.into())
				.narrow(true)
				.widget_instance(),
		]
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![TextAreaInput::new(self.to_string()).monospace(true).disabled(true).widget_instance()])]
	}
}

fn character_count_label(value: &str) -> String {
	let character_count = value.chars().count();

	format!("{character_count} Char{}", if character_count == 1 { "" } else { "s" })
}

/// Shortens a string to fit a table cell, cutting at the first line break or 40 characters with an ellipsis.
/// The 40 matches `truncate_breadcrumb_label`, so a preview reaching the trail isn't cut twice.
fn string_preview(value: &str) -> String {
	const MAX_CHARACTERS: usize = 40;

	let first_line = value.lines().next().unwrap_or_default();
	let cut_by_line_break = value.contains('\n');
	let cut_by_length = first_line.chars().count() > MAX_CHARACTERS;

	if !cut_by_line_break && !cut_by_length {
		return value.to_string();
	}

	first_line.chars().take(MAX_CHARACTERS - 1).chain(['…']).collect()
}

/// Shows text in a table cell as a string preview, with the full text as its tooltip when the preview cuts it short.
fn text_preview_label(text: String) -> WidgetInstance {
	let preview = string_preview(&text);
	let tooltip = if preview == text { String::new() } else { text };

	TextLabel::new(preview).narrow(true).tooltip_label(tooltip).widget_instance()
}

impl TableItemLayout for DVec2 {
	fn type_name() -> &'static str {
		"Vec2"
	}
	fn identifier(&self) -> String {
		"Vec2".to_string()
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![TextLabel::new(format_dvec2(*self)).narrow(true).widget_instance()]
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(self.value_widgets(PathStep::Element(0), _data))]
	}
}

impl TableItemLayout for Vec2 {
	fn type_name() -> &'static str {
		"Vec2"
	}
	fn identifier(&self) -> String {
		"Vec2".to_string()
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![TextLabel::new(format_dvec2(DVec2::new(self.x as f64, self.y as f64))).narrow(true).widget_instance()]
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(self.value_widgets(PathStep::Element(0), _data))]
	}
}

impl TableItemLayout for DAffine2 {
	fn type_name() -> &'static str {
		"Transform"
	}
	fn identifier(&self) -> String {
		"Transform".to_string()
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		transform_widgets(*self)
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(self.value_widgets(PathStep::Element(0), _data))]
	}
}

impl TableItemLayout for Affine2 {
	fn type_name() -> &'static str {
		"Transform"
	}
	fn identifier(&self) -> String {
		"Transform".to_string()
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		transform_widgets(DAffine2::from_cols_array(&self.to_cols_array().map(|x| x as f64)))
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(self.value_widgets(PathStep::Element(0), _data))]
	}
}

// Choice enums all display as their variant's label, shown inline as a plain text widget
macro_rules! impl_table_item_layout_for_choice_enum {
	($($ty:ty),* $(,)?) => {
		$(
			impl TableItemLayout for $ty {
				fn type_name() -> &'static str {
					stringify!($ty)
				}
				fn identifier(&self) -> String {
					self.to_string()
				}
				fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
					vec![TextLabel::new(self.to_string()).narrow(true).widget_instance()]
				}
				fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
					vec![LayoutGroup::row(self.value_widgets(PathStep::Element(0), _data))]
				}
			}
		)*
	}
}
impl_table_item_layout_for_choice_enum!(
	BlendMode,
	Cover,
	EffectKind,
	GlowSource,
	GradientForm,
	GradientSpread,
	GradientSpace,
	GradientHueDirection,
	GradientInterpolation,
	StrokeJoin,
	StrokeAlign,
	StrokeCap,
	WeldMethod,
	UnweldMethod,
	ExtrudeJoiningAlgorithm,
	PointSpacingType,
	StringCapitalization,
	TextDenomination,
	DesaturateMethod,
	RedGreenBlue,
	RelativeAbsolute,
	SelectiveColorChoice,
	TonalRange,
	AdjustmentChannel,
	HueSaturationRange,
	ScaleType,
	CentroidType,
	BooleanOperation,
	NoiseType,
	FractalType,
	CellularDistanceFunction,
	CellularReturnType,
	DomainWarpType,
	RealTimeMode,
	GridType,
	GridTopology,
	ArcType,
	SpiralType,
	TextAlign,
	QRCodeErrorCorrectionLevel,
	InterpolationDistribution,
	RowsOrColumns,
);

// ReferencePoint is not a choice enum with display labels, so its variant name serves as the label
impl TableItemLayout for ReferencePoint {
	fn type_name() -> &'static str {
		"ReferencePoint"
	}
	fn identifier(&self) -> String {
		format!("{self:?}")
	}
	fn value_widgets(&self, _target: PathStep, _data: &LayoutData) -> Vec<WidgetInstance> {
		vec![TextLabel::new(self.identifier()).narrow(true).widget_instance()]
	}
	fn value_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(self.value_widgets(PathStep::Element(0), _data))]
	}
}

/// Resolves the value/breadcrumb label for a `NodeId` against `network_interface` at the given `network_path`,
/// falling back to "Node {id}" if the node isn't present (e.g. an ID that no longer maps to a real node).
fn node_id_display_label(node_id: NodeId, network_interface: &NodeNetworkInterface, network_path: &[NodeId]) -> String {
	if network_interface.node_metadata(&node_id, network_path).is_some() {
		network_interface.display_name(&node_id, network_path)
	} else {
		format!("Node {node_id}")
	}
}

/// A reference to a node, as a button with its icon and name that selects it with the message from `select`.
pub fn node_reference_button(
	node_id: NodeId,
	network_interface: &NodeNetworkInterface,
	network_path: &[NodeId],
	selection_network_path: &[NodeId],
	select: impl Fn() -> Message + Send + Sync + 'static,
) -> TextButton {
	let is_layer = network_interface.is_layer(&node_id, network_path);
	let icon = network_interface.node_icon(&node_id, network_path);
	let name = network_interface.display_name(&node_id, network_path);
	let in_open_network = network_path == selection_network_path;
	let already_selected = network_interface.selected_nodes_in_nested_network(network_path).is_some_and(|selected| selected.0 == [node_id]);

	TextButton::new(name)
		.icon(icon)
		.disabled(!in_open_network || already_selected)
		.tooltip_label(if is_layer { "Select Layer" } else { "Select Node" })
		.tooltip_description(match (in_open_network, already_selected, is_layer) {
			(false, _, true) => "This layer is not in the part of the graph currently open.",
			(false, _, false) => "This node is not in the part of the graph currently open.",
			(true, true, true) => "This is already the selected layer.",
			(true, true, false) => "This is already the selected node.",
			(true, false, _) => "",
		})
		.on_update(move |_| select())
}

impl TableItemLayout for NodeId {
	fn type_name() -> &'static str {
		"NodeId"
	}
	fn identifier(&self) -> String {
		format!("Node {self}")
	}
	// Override so the breadcrumb uses the node's resolved display name instead of the bare-ID fallback `identifier()` returns
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		data.breadcrumbs.push(node_id_display_label(*self, data.network_interface, &data.node_lookup_network_path));
		self.value_page(data)
	}
	// Shown inline as a button with the node's icon and name that selects it. The lookups use
	// `data.node_lookup_network_path` (set by the enclosing `List<NodeId>` if rendering a path) so they resolve at any nesting depth.
	fn value_widgets(&self, _target: PathStep, data: &LayoutData) -> Vec<WidgetInstance> {
		let node_id = *self;
		let network_interface = data.network_interface;
		let network_path = data.node_lookup_network_path.clone();

		// An ID that no longer maps to a real node has nothing to show but itself
		if network_interface.node_metadata(&node_id, &network_path).is_none() {
			return vec![TextLabel::new(node_id_display_label(node_id, network_interface, &network_path)).widget_instance()];
		}

		// Selecting starts from the root of the newly selected node's data, since the current drill-down path means nothing there
		let select = move || Message::Batched {
			messages: Box::new([
				DataPanelMessage::TruncateElementPath { len: 0 }.into(),
				NodeGraphMessage::SelectedNodesSet { nodes: vec![node_id] }.into(),
			]),
		};
		vec![node_reference_button(node_id, network_interface, &network_path, data.selection_network_path, select).widget_instance()]
	}
	fn value_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(self.value_widgets(PathStep::Element(0), data))]
	}
}

/// Invokes another macro with the full list of `TableItemLayout`-implementing types whose values may appear
/// as attribute values. Both the value-rendering and drilldown-navigation dispatchers iterate this list,
/// so adding a new attribute-displayable type is a single edit here.
macro_rules! known_item_types {
	($apply:ident) => {
		$apply!(
			List<Artboard>,
			List<Graphic>,
			List<Vector>,
			List<Path>,
			List<Raster<CPU>>,
			List<Raster<GPU>>,
			List<Color>,
			List<Gradient>,
			List<String>,
			List<f64>,
			Gradient,
			Color,
			NodeId,
			DAffine2,
			DVec2,
			Affine2,
			Vec2,
			f64,
			u8,
			i64,
			bool,
			String,
			Vector,
			Path,
			BezPath,
			Raster<CPU>,
			Raster<GPU>,
			Graphic,
			Artboard,
			Appearance,
			Coverage,
			Cover,
			Effects,
			Effect,
			EffectKind,
			GlowSource,
			DashPattern,
			BoxCorners,
			TransferCurve,
			BlendMode,
			GradientForm,
			GradientSpread,
			GradientSpace,
			GradientHueDirection,
			GradientInterpolation,
			StrokeJoin,
			StrokeAlign,
			StrokeCap,
			WeldMethod,
			UnweldMethod,
			ExtrudeJoiningAlgorithm,
			PointSpacingType,
			StringCapitalization,
			TextDenomination,
			DesaturateMethod,
			RedGreenBlue,
			RelativeAbsolute,
			SelectiveColorChoice,
			TonalRange,
			AdjustmentChannel,
			HueSaturationRange,
			ScaleType,
			ReferencePoint,
			CentroidType,
			BooleanOperation,
			NoiseType,
			FractalType,
			CellularDistanceFunction,
			CellularReturnType,
			DomainWarpType,
			RealTimeMode,
			GridType,
			GridTopology,
			ArcType,
			SpiralType,
			TextAlign,
			QRCodeErrorCorrectionLevel,
			InterpolationDistribution,
			RowsOrColumns,
		);
	};
}

/// Uses `Display` instead of `Debug` for attribute types that have a nicer human-readable format.
fn display_value_override(any: &dyn Any) -> Option<String> {
	if let Some(value) = any.downcast_ref::<DVec2>() {
		return Some(format_dvec2(*value));
	}
	if let Some(value) = any.downcast_ref::<BlendMode>() {
		return Some(value.to_string());
	}
	if let Some(value) = any.downcast_ref::<PointId>() {
		return Some(if *value == PointId::UNNAMED { String::new() } else { value.inner().to_string() });
	}
	if let Some(value) = any.downcast_ref::<SegmentId>() {
		return Some(if *value == SegmentId::ZERO { String::new() } else { value.inner().to_string() });
	}
	if let Some(value) = any.downcast_ref::<CurveType>() {
		return Some(value.to_string());
	}
	None
}

/// Type-dispatched cell widgets for displaying an attribute value in a `List<T>` item.
/// Delegates to [`TableItemLayout::value_widgets`] so the same widget code is shared between
/// element-column rendering and attribute-column rendering. Returns `None` for unrecognized
/// types so the caller can fall back to a debug-formatted [`TextLabel`].
fn dispatch_value_widgets(any: &dyn Any, target: PathStep, data: &LayoutData) -> Option<Vec<WidgetInstance>> {
	// `NodeIdPath` (e.g. the `editor:layer_path` attribute) drills into its inner path list, matching `drilldown_attribute_layout`.
	if let Some(path) = any.downcast_ref::<NodeIdPath>() {
		return Some(path.0.value_widgets(target, data));
	}
	macro_rules! check {
		( $($ty:ty),* $(,)? ) => {
			$(
				if let Some(value) = any.downcast_ref::<$ty>() {
					return Some(value.value_widgets(target, data));
				}
			)*
		};
	}
	known_item_types!(check);
	None
}

/// Renders a `List<NodeId>` as a path: the standard table view, but each item's `NodeId` value is resolved
/// against the network path made up of all preceding items. So for a path `[outer, middle, leaf]`, item 0
/// resolves at root, item 1 resolves at `[outer]`, and item 2 resolves at `[outer, middle]` — letting deeply
/// nested layers display each step's correct name. Drilling into an item drops into that node's value page
/// using the same prefix as `network_path`.
fn table_node_id_path_layout_with_breadcrumb(path: &List<NodeId>, data: &mut LayoutData) -> Vec<LayoutGroup> {
	data.breadcrumbs.push(path.identifier());

	if let Some(step) = data.desired_path.get(data.current_depth).cloned() {
		if let PathStep::Element(index) = step
			&& let Some(node_id) = path.element(index)
		{
			let prefix: Vec<NodeId> = path.iter_element_values().take(index).copied().collect();
			let saved = std::mem::replace(&mut data.node_lookup_network_path, prefix);
			data.current_depth += 1;
			let result = node_id.layout_with_breadcrumb(data);
			data.current_depth -= 1;
			data.node_lookup_network_path = saved;
			return result;
		}
		warn!("Desired path truncated");
		data.desired_path.truncate(data.current_depth);
	}

	let mut rows = (0..path.len())
		.map(|index| {
			let node_id = path.element(index).unwrap();
			let prefix: Vec<NodeId> = path.iter_element_values().take(index).copied().collect();
			let saved = std::mem::replace(&mut data.node_lookup_network_path, prefix);
			let widgets = node_id.value_widgets(PathStep::Element(index), data);
			data.node_lookup_network_path = saved;
			vec![vec![TextLabel::new(format!("{index}")).narrow(true).widget_instance()], widgets]
		})
		.collect::<Vec<_>>();
	rows.insert(0, single_widget_cells(column_headings(&["", "element"])));

	vec![LayoutGroup::table_of_cells(rows, false)]
}

/// Type-dispatched recursion into an attribute value for the Data panel breadcrumb navigation.
/// Mirrors [`dispatch_value_widget`] but routes to [`TableItemLayout::layout_with_breadcrumb`].
/// Returns `None` for unrecognized types.
fn drilldown_attribute_layout(any: &dyn Any, data: &mut LayoutData) -> Option<Vec<LayoutGroup>> {
	// `NodeIdPath` is interpreted as a path (e.g. the `editor:layer_path` attribute), so each item's NodeId value
	// resolves against the prefix made up of preceding items. Handled before the generic blanket impl.
	if let Some(path) = any.downcast_ref::<NodeIdPath>() {
		return Some(table_node_id_path_layout_with_breadcrumb(&path.0, data));
	}
	macro_rules! check {
		( $($ty:ty),* $(,)? ) => {
			$(
				if let Some(value) = any.downcast_ref::<$ty>() {
					return Some(value.layout_with_breadcrumb(data));
				}
			)*
		};
	}
	known_item_types!(check);
	None
}

/// Decomposes a transform into location, rotation, and scale, each headed by the icon of its Layer menu action.
fn transform_widgets(transform: DAffine2) -> Vec<WidgetInstance> {
	let (scale, angle, translation) = if transform.matrix2.determinant().abs() <= f64::EPSILON {
		let [col_0, col_1] = transform.matrix2.to_cols_array_2d().map(|[x, y]| DVec2::new(x, y));

		let scale = DVec2::new(col_0.length(), col_1.length());

		let rotation = if scale.x > f64::EPSILON {
			col_0.y.atan2(col_0.x)
		} else if scale.y > f64::EPSILON {
			col_1.y.atan2(col_1.x) - std::f64::consts::FRAC_PI_2
		} else {
			0.
		};

		(scale, rotation, transform.translation)
	} else {
		transform.to_scale_angle_translation()
	};

	let location_text = format!("({} px, {} px)", format_rounded(translation.x, 3), format_rounded(translation.y, 3));
	let rotation_text = format!("{}°", format_rounded(angle.to_degrees(), 3));
	let scale_text = format!("({}x, {}x)", format_rounded(scale.x, 3), format_rounded(scale.y, 3));

	vec![
		IconLabel::new("TransformationGrab").tooltip_label("Location").widget_instance(),
		Separator::new(SeparatorStyle::Related).widget_instance(),
		TextLabel::new(location_text).narrow(true).widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		IconLabel::new("TransformationRotate").tooltip_label("Rotation").widget_instance(),
		Separator::new(SeparatorStyle::Related).widget_instance(),
		TextLabel::new(rotation_text).narrow(true).widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		IconLabel::new("TransformationScale").tooltip_label("Scale").widget_instance(),
		Separator::new(SeparatorStyle::Related).widget_instance(),
		TextLabel::new(scale_text).narrow(true).widget_instance(),
	]
}

fn format_dvec2(value: DVec2) -> String {
	format!("({} px, {} px)", format_rounded(value.x, 3), format_rounded(value.y, 3))
}
