use glam::{DAffine2, IVec2};
use graphene_core::instances::{Instance, Instances};
use graphene_core::raster_types::{CPU, GPU, Raster, RasterDataTable};
use graphene_core::transform::TransformMut;
use graphene_core::uuid::NodeId;
use graphene_core::vector::{VectorData, VectorDataTable};
use graphene_core::{AlphaBlending, Artboard, ArtboardGroupTable, CloneVarArgs, Color, Context, Ctx, ExtractAll, GraphicElement, GraphicGroupTable, OwnedContextImpl};

#[node_macro::node(category(""))]
async fn layer<I: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>, RasterDataTable<GPU>)] mut stack: Instances<I>,
	#[implementations(GraphicElement, VectorData, Raster<CPU>, Raster<GPU>)] element: I,
	node_path: Vec<NodeId>,
) -> Instances<I> {
	// Get the penultimate element of the node path, or None if the path is too short
	let source_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	stack.push(Instance {
		instance: element,
		transform: DAffine2::IDENTITY,
		alpha_blending: AlphaBlending::default(),
		source_node_id,
	});

	stack
}

#[node_macro::node(category("Debug"))]
async fn to_element<Data: Into<GraphicElement> + 'n>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
	 	VectorDataTable,
		RasterDataTable<CPU>,
	 	RasterDataTable<GPU>,
	)]
	data: Data,
) -> GraphicElement {
	data.into()
}

#[node_macro::node(category("General"))]
async fn to_group<Data: Into<GraphicGroupTable> + 'n>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
		VectorDataTable,
		RasterDataTable<CPU>,
		RasterDataTable<GPU>,
	)]
	element: Data,
) -> GraphicGroupTable {
	element.into()
}

#[node_macro::node(category("General"))]
async fn flatten_group(_: impl Ctx, group: GraphicGroupTable, fully_flatten: bool) -> GraphicGroupTable {
	// TODO: Avoid mutable reference, instead return a new GraphicGroupTable?
	fn flatten_group(output_group_table: &mut GraphicGroupTable, current_group_table: GraphicGroupTable, fully_flatten: bool, recursion_depth: usize) {
		for current_instance in current_group_table.instance_ref_iter() {
			let current_element = current_instance.instance.clone();
			let reference = *current_instance.source_node_id;

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any GraphicGroups we encounter
				GraphicElement::GraphicGroup(mut current_element) if recurse => {
					// Apply the parent group's transform to all child elements
					for graphic_element in current_element.instance_mut_iter() {
						*graphic_element.transform = *current_instance.transform * *graphic_element.transform;
					}

					flatten_group(output_group_table, current_element, fully_flatten, recursion_depth + 1);
				}
				// Handle any leaf elements we encounter, which can be either non-GraphicGroup elements or GraphicGroups that we don't want to flatten
				_ => {
					output_group_table.push(Instance {
						instance: current_element,
						transform: *current_instance.transform,
						alpha_blending: *current_instance.alpha_blending,
						source_node_id: reference,
					});
				}
			}
		}
	}

	let mut output = GraphicGroupTable::default();
	flatten_group(&mut output, group, fully_flatten, 0);

	output
}

#[node_macro::node(category("Vector"))]
async fn flatten_vector(_: impl Ctx, group: GraphicGroupTable) -> VectorDataTable {
	// TODO: Avoid mutable reference, instead return a new GraphicGroupTable?
	fn flatten_group(output_group_table: &mut VectorDataTable, current_group_table: GraphicGroupTable) {
		for current_instance in current_group_table.instance_ref_iter() {
			let current_element = current_instance.instance.clone();
			let reference = *current_instance.source_node_id;

			match current_element {
				// If we're allowed to recurse, flatten any GraphicGroups we encounter
				GraphicElement::GraphicGroup(mut current_element) => {
					// Apply the parent group's transform to all child elements
					for graphic_element in current_element.instance_mut_iter() {
						*graphic_element.transform = *current_instance.transform * *graphic_element.transform;
					}

					flatten_group(output_group_table, current_element);
				}
				// Handle any leaf elements we encounter, which can be either non-GraphicGroup elements or GraphicGroups that we don't want to flatten
				GraphicElement::VectorData(vector_instance) => {
					for current_element in vector_instance.instance_ref_iter() {
						output_group_table.push(Instance {
							instance: current_element.instance.clone(),
							transform: *current_instance.transform * *current_element.transform,
							alpha_blending: AlphaBlending {
								blend_mode: current_element.alpha_blending.blend_mode,
								opacity: current_instance.alpha_blending.opacity * current_element.alpha_blending.opacity,
								fill: current_element.alpha_blending.fill,
								clip: current_element.alpha_blending.clip,
							},
							source_node_id: reference,
						});
					}
				}
				_ => {}
			}
		}
	}

	let mut output = VectorDataTable::default();
	flatten_group(&mut output, group);

	output
}

#[node_macro::node(category(""))]
async fn to_artboard<Data: Into<GraphicGroupTable> + 'n>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> GraphicGroupTable,
		Context -> VectorDataTable,
		Context -> RasterDataTable<CPU>,
		Context -> RasterDataTable<GPU>,
	)]
	contents: impl Node<Context<'static>, Output = Data>,
	label: String,
	location: IVec2,
	dimensions: IVec2,
	background: Color,
	clip: bool,
) -> Artboard {
	let footprint = ctx.try_footprint().copied();
	let mut new_ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.translate(location.as_dvec2());
		new_ctx = new_ctx.with_footprint(footprint);
	}
	let graphic_group = contents.eval(new_ctx.into_context()).await;

	Artboard {
		graphic_group: graphic_group.into(),
		label,
		location: location.min(location + dimensions),
		dimensions: dimensions.abs(),
		background,
		clip,
	}
}

#[node_macro::node(category(""))]
async fn append_artboard(_ctx: impl Ctx, mut artboards: ArtboardGroupTable, artboard: Artboard, node_path: Vec<NodeId>) -> ArtboardGroupTable {
	// Get the penultimate element of the node path, or None if the path is too short.
	// This is used to get the ID of the user-facing "Artboard" node (which encapsulates this internal "Append Artboard" node).
	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	artboards.push(Instance {
		instance: artboard,
		transform: DAffine2::IDENTITY,
		alpha_blending: AlphaBlending::default(),
		source_node_id: encapsulating_node_id,
	});

	artboards
}
