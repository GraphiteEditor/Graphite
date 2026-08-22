use core_types::attribute::{Attr, EditorLayerPath, Transform as TransformAttr};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::extent::{ExtentIn, LevelIn, ListIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt, Level};
use core_types::list::{AttributeDyn, AttributeValueDyn, Item, List, ListDyn};
use core_types::registry::types::{Angle, SignedInteger};
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_LAYER_PATH, ATTR_TRANSFORM, AnyHash, BlendMode, CacheHash, Color, Context, Ctx, DeriveCtx, ExtractIndex, InjectIndex};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicList};
use graphic_types::{ATTR_EDITOR_MERGED_LAYERS, Artboard, Vector};
use raster_types::{CPU, GPU, Raster};
use vector_types::gradient::{GradientSpreadMethod, GradientType};
use vector_types::{GradientStop, GradientStops, ReferencePoint};

/// Resolves a signed index over `total` lanes: negatives count from the end,
/// out of range resolves to nothing.
fn resolve_index(index: f64, total: u64) -> Option<u64> {
	let index = index as i64;
	match index < 0 {
		true => total.checked_sub(index.unsigned_abs()),
		false => ((index as u64) < total).then_some(index as u64),
	}
}

/// Returns a one-lane level holding the item at the specified index with its
/// attributes, or an empty level when the index is out of range.
#[node_macro::node(category("General"), extent(index_elements_extent))]
pub fn index_elements<T>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The list of data.
	list: impl Node<Context<'_>, Output = T>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> Result<T, Interrupt> {
	let total = match list.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("index elements over a non-exact extent").into()),
	};
	let Some(source) = resolve_index(index, total) else {
		return Err(GraphError::new("index elements addressed its empty selection").into());
	};
	let mut shifted = *ctx;
	shifted.set_index(source);
	list.eval(&shifted)
}

fn index_elements_extent(list: ExtentIn<'_>, index: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => index.get().zip(list.at(level)).map(|(index, extent)| match extent {
			Extent::Exactly(count) => Extent::Exactly(resolve_index(index, count as u64).is_some() as usize),
			_ => Extent::Exactly(1),
		}),
		false => list.at(level),
	}
}

/// Returns the list with the element at the specified index removed.
/// If no value exists at that index, the list is returned unchanged.
#[node_macro::node(category("General"), extent(omit_element_extent))]
pub fn omit_element<T>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The list of data.
	list: impl Node<Context<'_>, Output = T>,
	/// The index of the item to remove, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> Result<T, Interrupt> {
	let total = match list.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("omit over a non-exact extent").into()),
	};
	let lane = ctx.innermost_index();
	let source = match resolve_index(index, total) {
		Some(omitted) if lane >= omitted => lane + 1,
		_ => lane,
	};
	let mut shifted = *ctx;
	shifted.set_index(source);
	list.eval(&shifted)
}

fn omit_element_extent(list: ExtentIn<'_>, index: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => index.get().zip(list.at(level)).map(|(index, extent)| match extent {
			Extent::Exactly(count) if resolve_index(index, count as u64).is_some() => Extent::Exactly(count - 1),
			extent => extent,
		}),
		false => list.at(level),
	}
}

/// Returns the bare element (without the item's attributes) at the specified index in a `List`.
/// Use this when downstream nodes want just the inner value rather than a `List` containing a single item.
/// If no value exists at that index, the element type's default is returned.
#[node_macro::node(category("General"))]
pub fn extract_element<T: Clone + Default + Send + Sync + CacheHash + 'static>(
	_: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The `List` of data to extract from.
	#[implementations(String, f64, NodeId, Color, GradientStops, Vector, Raster<CPU>, Graphic, Artboard)] list: IList<T>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> T {
	resolve_index(index, list.len() as u64).map(|resolved| list.element_ref(resolved as usize).clone()).unwrap_or_default()
}

/// One subgraph invocation per content row, the row riding as a vararg, with
/// the subgraph's lanes concatenated into one flat level. The level reports a
/// lower bound; consumers drain to the past-end signal.
#[node_macro::node(category("General"))]
fn map<Row: Clone + Send + Sync + CacheHash + 'static, T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex + InjectIndex + Copy,
	#[implementations(Graphic, Vector, Raster<CPU>, Color, GradientStops, String)] content: IList<Row>,
	mapped: impl Node<Context<'_>, Output = IList<T>>,
) -> Result<IList<T>, Interrupt> {
	let mut remaining = ctx.innermost_index();
	for row in 0..content.len() {
		let item = crate::record::vararg_row(content, row);
		let scoped = ctx.push_vararg(&item);
		let lanes = mapped.inner_extent_at(&scoped.ctx(), row as u64)?;
		if remaining >= lanes {
			remaining -= lanes;
			continue;
		}
		let mut frame = core_types::context::IndexLink { index: 0, outer: None };
		return mapped.eval(&scoped.ctx().push_level(&mut frame, row as u64, remaining));
	}
	Err(GraphError::past_end().into())
}

#[node_macro::node(category("General"))]
fn mirror<T: Send + Clone>(
	_: impl Ctx,
	#[implementations(
		List<Graphic>,
		List<Vector>,
		List<String>,
		List<Raster<CPU>>,
		List<Color>,
		List<GradientStops>,
	)]
	content: List<T>,
	#[default(ReferencePoint::Center)] relative_to_bounds: ReferencePoint,
	#[unit(" px")] offset: f64,
	#[range]
	#[soft(-90..90)]
	angle: Angle,
	#[default(true)] keep_original: bool,
) -> List<T>
where
	List<T>: BoundingBox,
{
	// Normalize the direction vector
	let normal = DVec2::from_angle(angle.to_radians());

	// The mirror reference may be based on the bounding box if an explicit reference point is chosen
	let RenderBoundingBox::Rectangle(bounding_box) = content.bounding_box(DAffine2::IDENTITY, false) else {
		return content;
	};

	let reference_point_location = relative_to_bounds.point_in_bounding_box((bounding_box[0], bounding_box[1]).into());
	let mirror_reference_point = reference_point_location.map(|point| point + normal * offset);

	// Create the reflection matrix
	let reflection = DAffine2::from_mat2_translation(
		glam::DMat2::from_cols(
			DVec2::new(1. - 2. * normal.x * normal.x, -2. * normal.y * normal.x),
			DVec2::new(-2. * normal.x * normal.y, 1. - 2. * normal.y * normal.y),
		),
		DVec2::ZERO,
	);

	// Apply reflection around the reference point
	let reflected_transform = if let Some(mirror_reference_point) = mirror_reference_point {
		DAffine2::from_translation(mirror_reference_point) * reflection * DAffine2::from_translation(-mirror_reference_point)
	} else {
		reflection * DAffine2::from_translation(DVec2::from_angle(angle.to_radians()) * DVec2::splat(-offset))
	};

	let mut result_list = List::new();

	// Add original items depending on the keep_original flag
	if keep_original {
		for item in content.clone().into_iter() {
			result_list.push(item);
		}
	}

	// Create and add mirrored items
	for mut row in content.into_iter() {
		let current_transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
		row.set_attribute(ATTR_TRANSFORM, reflected_transform * current_transform);
		result_list.push(row);
	}

	result_list
}

/// Returns the path identifying the subgraph (network) that contains this proto node — i.e. the input `node_path`
/// with its own trailing entry dropped. The terminating element of the returned path is the document node whose
/// encapsulated network we live in, so the path doubles as a unique reference to that node at any nesting depth.
/// Used as the value source for stamping the `editor:layer_path` attribute on each item of a layer's output, which lets
/// editor tools (e.g. selection, click target routing) trace data back to its owning layer regardless of whether
/// the layer is at the root document network or nested inside a custom subgraph.
#[node_macro::node(name("Path of Subgraph"), category(""))]
pub fn path_of_subgraph(_: impl Ctx, node_path: Vec<NodeId>) -> Vec<NodeId> {
	let len = node_path.len();
	node_path.into_iter().take(len.saturating_sub(1)).collect()
}

/// The layer-path stamp: writes the owning layer's document node path on
/// each lane, which lets editor tools trace data back to its layer.
#[node_macro::node(category(""))]
pub fn stamp_layer_path<'e, T>(ctx: impl Ctx + ExtractArena<'e>, element: T, path: Vec<NodeId>) -> Result<(T, Attr<'e, EditorLayerPath>), Interrupt> {
	let (parked, _) = ctx.arena().alloc(path).ok_or(GraphError {
		kind: core_types::gpoll::ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;
	Ok((element, Attr(parked.as_slice())))
}

/// Sets a named attribute on the input `List`, computing one value per item via the value-producing input. That input
/// is evaluated once per item, with the item's index and the item itself (as a `List` containing only that item,
/// passed as a vararg) provided via context, so the upstream pipeline can return a different value per item that may
/// be derived from the item's own data. If the attribute already exists, its values are replaced; if not, it's added.
/// The value is type-erased into an `AttributeValueDyn` by an auto-inserted convert node, so this node only
/// monomorphizes over `T` instead of the cartesian product `(T, U)`.
#[node_macro::node(category("Attributes: Write"))]
fn write_attribute<T: AnyHash + Clone + Send + Sync + CacheHash>(
	ctx: impl Ctx + DeriveCtx,
	/// The `List` to set the named attribute on (one value per item).
	#[implementations(
		List<Artboard>,
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Color>,
		List<GradientStops>,
		List<f64>,
		List<bool>,
		List<String>,
		List<DAffine2>,
		List<BlendMode>,
		List<GradientType>,
		List<GradientSpreadMethod>,
	)]
	mut content: List<T>,
	/// The attribute name (key) to write or replace.
	name: String,
	/// The node that produces the attribute value for each item. Called once per item with the item's index in context.
	#[implementations(Context -> AttributeValueDyn)]
	value: impl Node<Context<'_>, Output = AttributeValueDyn>,
) -> Result<List<T>, Interrupt> {
	let spilled = ctx.index_head();
	for index in 0..content.len() {
		let row = content.clone_item(index).expect("index is within bounds");
		let item = List::new_from_item(row);
		let scoped = ctx.push_vararg(&item);
		let v = value.eval(&scoped.ctx().promoted(&spilled, index as u64))?;
		content.set_attribute_value_dyn(&name, index, v);
	}
	Ok(content)
}

/// Sets a named attribute on the primary list, with each value taken from the corresponding item's element in the source list (paired by index, wrapping if the source has fewer items).
/// The source is type-erased into an `AttributeDyn` by an auto-inserted convert node, so this node only monomorphizes over `T` instead of the cartesian product `(T, U)`.
#[node_macro::node(category("Attributes: Write"))]
fn attach_attribute<T: AnyHash + Clone + Send + Sync + CacheHash>(
	_: impl Ctx,
	/// The `List` to attach the new attribute to.
	#[implementations(
		List<Artboard>,
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Color>,
		List<GradientStops>,
		List<f64>,
		List<bool>,
		List<String>,
		List<DAffine2>,
		List<BlendMode>,
		List<GradientType>,
		List<GradientSpreadMethod>,
	)]
	mut content: List<T>,
	/// The source values to attach.
	#[expose]
	source: AttributeDyn,
	/// The name to assign to the new destination attribute.
	name: String,
) -> List<T> {
	if source.is_empty() {
		return content;
	}
	content.set_attribute_dyn(name, source);
	content
}

/// Reads a named `Vector` attribute from the input list, outputting each value as an element of a new `Vector[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_vector(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<Vector> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<Vector>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Reads a named numeric attribute (`f64`, `u64`, or `u32`) from the input list, outputting each value as an element of a new `f64[]`. Integer values are converted to `f64`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_number(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<f64> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let value = content
			.attribute::<f64>(&name, index)
			.copied()
			.or_else(|| content.attribute::<u64>(&name, index).map(|v| *v as f64))
			.or_else(|| content.attribute::<u32>(&name, index).map(|v| *v as f64));
		let Some(value) = value else { continue };
		result.push(Item::new_from_element(value));
	}
	result
}

/// Reads a named `bool` attribute from the input list, outputting each value as an element of a new `bool[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_bool(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<bool> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<bool>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `String` attribute from the input list, outputting each value as an element of a new `String[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_string(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<String> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<String>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Reads a named `DAffine2` transform attribute from the input list, outputting each value as an element of a new `DAffine2[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_transform(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<DAffine2> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<DAffine2>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `Color` attribute from the input list, outputting each value as an element of a new `Color[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_color(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<Color> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<Color>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `BlendMode` attribute from the input list, outputting each value as an element of a new `BlendMode[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_blend_mode(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<BlendMode> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<BlendMode>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `GradientType` attribute from the input list, outputting each value as an element of a new `GradientType[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_gradient_type(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<GradientType> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<GradientType>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `GradientSpreadMethod` attribute from the input list, outputting each value as an element of a new `GradientSpreadMethod[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_spread_method(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<GradientSpreadMethod> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<GradientSpreadMethod>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `GradientStops` attribute from the input list, outputting each value as an element of a new `GradientStops[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_gradient_stops(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<GradientStops> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<GradientStops>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Reads a named `Artboard` attribute from the input list, outputting each value as an element of a new `Artboard[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_artboard(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<Artboard> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<Artboard>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Reads a named `Raster` attribute from the input list, outputting each value as an element of a new `Raster[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_raster(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: String,
) -> List<Raster<CPU>> {
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<Raster<CPU>>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Joins two levels of the same type, the base's lanes followed by the new's.
#[node_macro::node(category("General"), extent(extend_extent))]
pub fn extend<T>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The wire whose lanes appear at the start of the extended level.
	base: impl Node<Context<'_>, Output = T>,
	/// The wire whose lanes appear at the end of the extended level.
	#[expose]
	new: impl Node<Context<'_>, Output = T>,
) -> Result<T, Interrupt> {
	let split = match base.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		// A scalar side joins the concat as a single lane, per `Extent::sum`.
		GPoll::Final(Extent::Free) => 1,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("extend over a non-exact base extent").into()),
	};
	let lane = ctx.innermost_index();
	match lane < split {
		true => base.eval(ctx),
		false => {
			let mut shifted = *ctx;
			shifted.set_index(lane - split);
			new.eval(&shifted)
		}
	}
}

/// The top level sums both sides; inner levels must agree (rectangular), a
/// free side or a side with no top-level lanes defers to the other.
fn extend_extent(base: ExtentIn<'_>, new: ExtentIn<'_>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => Extent::sum(base.at(level), new.at(level)),
		false => base.at(level).zip(new.at(level)).and_then(|extents| match extents {
			(Extent::Free, other) | (other, Extent::Free) => GPoll::Final(other),
			(base_inner, new_inner) if base_inner == new_inner => GPoll::Final(base_inner),
			(base_inner, new_inner) => {
				let top = LevelIn {
					level: level.depth - 1,
					depth: level.depth,
				};
				match (base.at(top), new.at(top)) {
					(GPoll::Final(Extent::Exactly(0)), _) => GPoll::Final(new_inner),
					(_, GPoll::Final(Extent::Exactly(0))) => GPoll::Final(base_inner),
					_ => GPoll::error("extend inner extents differ"),
				}
			}
		}),
	}
}

// TODO: Eventually remove this document upgrade code
/// Performs an obsolete function as part of a migration from an older document format.
/// Users are advised to delete this node and replace it with a new one.
#[node_macro::node(category(""))]
pub fn legacy_layer_extend<T: Send + Clone>(
	_: impl Ctx,
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<GradientStops>)] base: List<T>,
	#[expose]
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<GradientStops>)]
	new: List<T>,
	nested_node_path: List<NodeId>,
) -> List<T> {
	// Get the penultimate element of the node path, or None if the path is too short
	// This is used to get the ID of the user-facing parent layer-style node (which encapsulates this internal node).
	let layer = {
		let index = nested_node_path.len().wrapping_sub(2);
		nested_node_path.element(index).copied()
	};

	let mut base = base;
	for mut row in new.into_iter() {
		row.set_attribute(ATTR_EDITOR_LAYER_PATH, layer);
		base.push(row);
	}

	base
}

/// Nests the input graphical content in a wrapper graphic. This essentially "groups" the input.
/// The inverse of this node is 'Flatten Graphic'.
#[node_macro::node(category("General"), extent(wrap_graphic_extent))]
pub fn wrap_graphic(_: impl Ctx + ExtractIndex + InjectIndex + Copy, content: IList<Graphic>) -> Result<IList<Graphic>, Interrupt> {
	// SAFETY: a materialized input's frames are arena-resident.
	let item = unsafe { core_types::record::GroupItem::from_resident(content.batch()) };
	Ok(Graphic::Group(core_types::record::Group {
		row: None,
		content: core_types::record::GroupContent::Run(item),
	}))
}

/// The collected group is the level's single lane.
fn wrap_graphic_extent(_content: ListIn<'_, Graphic>, _level: LevelIn) -> GPoll<Extent> {
	GPoll::Final(Extent::Exactly(1))
}

/// Converts the level's elements into `Graphic` elements. A `Graphic` level passes through
/// unchanged. The legacy list rows accept an unconverted producer's list value as one element.
#[node_macro::node(category("General"))]
pub fn to_graphic<T: Into<Graphic> + Clone + Send + Sync + core_types::CacheHash + 'static>(
	_: impl Ctx,
	#[implementations(
		Graphic,
		Vector,
		Raster<CPU>,
		Raster<GPU>,
		Color,
		GradientStops,
		String,
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<GradientStops>,
		List<String>,
	)]
	content: T,
) -> Graphic {
	content.into()
}

/// The transitional level bridge: the wire's records as the legacy list an
/// unconverted consumer expects, attributes copied through their erased
/// reads. Registered under the legacy convert identifiers; the rows die with
/// the last legacy consumer.
#[node_macro::node(category(""))]
pub fn level_to_list<T: Clone + Send + Sync + CacheHash + 'static>(
	_: impl Ctx + ExtractIndex + InjectIndex + Copy,
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, GradientStops, String)] value: IList<T>,
	_converter: (),
) -> List<T> {
	// SAFETY: a materialized input's frames are arena-resident.
	let item = unsafe { core_types::record::GroupItem::from_resident(value.batch()) };
	graphic_types::graphic::run_to_render_list::<T>(&item).expect("the run holds the row's element type")
}

pub use _level_to_list_mod::level_to_list_entries;
pub use _to_graphic_mod::to_graphic_entries;

/// Removes a level of nesting from a `Graphic[]`, or all nesting if "Fully Flatten" is enabled.
#[node_macro::node(category("General"), extent(flatten_graphic_extent))]
pub fn flatten_graphic(ctx: impl Ctx + ExtractIndex + InjectIndex + Copy, content: IList<Graphic>, fully_flatten: bool) -> Result<IList<(Graphic, Attr<TransformAttr>)>, Interrupt> {
	let mut remaining = ctx.innermost_index() as usize;
	for row in 0..content.len() {
		let graphic = content.element_ref(row);
		let count = crate::record::leaf_count(graphic, fully_flatten, 0);
		if remaining >= count {
			remaining -= count;
			continue;
		}
		let transform: DAffine2 = content.lane(row).attr::<TransformAttr>();
		if let Some((leaf, composed)) = crate::record::locate(graphic, transform, fully_flatten, 0, &mut remaining) {
			return Ok((leaf, Attr(composed)));
		}
	}
	Err(GraphError::new("flatten addressed past its leaf count").into())
}

/// The level holds one row per leaf of the walk.
fn flatten_graphic_extent(content: ListIn<'_, Graphic>, fully_flatten: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => fully_flatten
			.get()
			.zip(content.get())
			.map(|(fully_flatten, content)| Extent::Exactly((0..content.len()).map(|row| crate::record::leaf_count(content.element_ref(row), fully_flatten, 0)).sum())),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Converts a `Graphic[]` into a `Vector[]` by deeply flattening any vector content it contains, and discarding any non-vector content.
#[node_macro::node(category("Vector"))]
pub fn flatten_vector<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Vector>)] content: T) -> List<Vector> {
	let graphic_list = content.into_graphic_list();
	let mut output: List<Vector> = graphic_list.clone().into_flattened_list();

	// TODO: Replace this snapshot hack with per-layer metadata driven by each layer's Monitor node.
	// TODO: Flattening here erases the upstream `List<Graphic>` hierarchy that editor metadata collection walks
	// TODO: to populate `upstream_footprints` / `local_transforms` / `click_targets` per child layer. As a workaround
	// TODO: we stash the pre-flattened list on the output so `List<Vector>::collect_metadata` can recurse into it,
	// TODO: which conflates render output with editor metadata and forces the pre-compensation dance below.
	// TODO: The cleaner fix is to drive each layer's metadata from its own Monitor's captured `(Context, List<Graphic>)`,
	// TODO: at which point this attribute (and the equivalents in Boolean Operation, Solidify Stroke, Flatten Path,
	// TODO: Morph, Rasterize) become unnecessary.
	if !output.is_empty() {
		// Item 0 carries a composed transform inherited from the flattened input, but the merged_layers
		// already holds the original transforms; pre-compensate by item 0's inverse so the renderer's
		// `upstream_footprint *= item_0_transform` recursion cancels out and leaves the originals intact.
		let mut graphic_list = graphic_list;
		let item_0_transform: DAffine2 = output.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		if item_0_transform.matrix2.determinant().abs() > f64::EPSILON {
			let inverse = item_0_transform.inverse();
			for transform in graphic_list.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
				*transform = inverse * *transform;
			}
		}

		output.set_attribute(ATTR_EDITOR_MERGED_LAYERS, 0, graphic_list);
	}

	output
}

/// Converts a `Graphic[]` into a `Raster[]` by deeply flattening any raster content it contains, and discarding any non-raster content.
#[node_macro::node(category("Raster"))]
pub fn flatten_raster<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Raster<CPU>>)] content: T) -> List<Raster<CPU>> {
	content.into_flattened_list()
}

/// Converts a `Graphic[]` into a `Color[]` by deeply flattening any color content it contains, and discarding any non-color content.
#[node_macro::node(category("General"))]
pub fn flatten_color<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Color>)] content: T) -> List<Color> {
	content.into_flattened_list()
}

/// Converts a `Graphic[]` into a `GradientStops[]` by deeply flattening any gradient content it contains, and discarding any non-gradient content.
#[node_macro::node(category("General"))]
pub fn flatten_gradient<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<GradientStops>)] content: T) -> List<GradientStops> {
	content.into_flattened_list()
}

/// Constructs a gradient from a `Color[]`, where the colors are evenly distributed as gradient stops across the range from 0 to 1.
#[node_macro::node(category("Color"))]
fn colors_to_gradient(_: impl Ctx + ExtractIndex + InjectIndex + Copy, colors: IList<Color>) -> GradientStops {
	let stop = |position: f64, color: Color| GradientStop { position, midpoint: 0.5, color };
	match colors.len() {
		0 => GradientStops::new(vec![stop(0., Color::BLACK), stop(1., Color::BLACK)]),
		1 => GradientStops::new(vec![stop(0., colors.get(0)), stop(1., colors.get(0))]),
		total => GradientStops::new((0..total).map(|index| stop(index as f64 / (total - 1) as f64, colors.get(index)))),
	}
}
