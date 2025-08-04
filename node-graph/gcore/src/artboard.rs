use crate::blending::AlphaBlending;
use crate::bounds::BoundingBox;
use crate::math::quad::Quad;
use crate::raster_types::{CPU, GPU, Raster};
use crate::table::{Table, TableRow};
use crate::transform::TransformMut;
use crate::uuid::NodeId;
use crate::vector::Vector;
use crate::{CloneVarArgs, Color, Context, Ctx, ExtractAll, Graphic, OwnedContextImpl};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, IVec2};
use std::hash::Hash;

/// Some [`ArtboardData`] with some optional clipping bounds that can be exported.
#[derive(Clone, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct Artboard {
	pub graphic_group: Table<Graphic>,
	pub label: String,
	pub location: IVec2,
	pub dimensions: IVec2,
	pub background: Color,
	pub clip: bool,
}

impl Default for Artboard {
	fn default() -> Self {
		Self::new(IVec2::ZERO, IVec2::new(1920, 1080))
	}
}

impl Artboard {
	pub fn new(location: IVec2, dimensions: IVec2) -> Self {
		Self {
			graphic_group: Table::new(),
			label: "Artboard".to_string(),
			location: location.min(location + dimensions),
			dimensions: dimensions.abs(),
			background: Color::WHITE,
			clip: false,
		}
	}
}

impl BoundingBox for Artboard {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		let artboard_bounds = (transform * Quad::from_box([self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()])).bounding_box();
		if self.clip {
			Some(artboard_bounds)
		} else {
			[self.graphic_group.bounding_box(transform, include_stroke), Some(artboard_bounds)]
				.into_iter()
				.flatten()
				.reduce(Quad::combine_bounds)
		}
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_artboard_group<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Artboard>, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Default, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
	pub struct ArtboardGroup {
		pub artboards: Vec<(Artboard, Option<NodeId>)>,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum EitherFormat {
		ArtboardGroup(ArtboardGroup),
		ArtboardGroupTable(Table<Artboard>),
	}

	Ok(match EitherFormat::deserialize(deserializer)? {
		EitherFormat::ArtboardGroup(artboard_group) => {
			let mut table = Table::new();
			for (artboard, source_node_id) in artboard_group.artboards {
				table.push(TableRow {
					element: artboard,
					transform: DAffine2::IDENTITY,
					alpha_blending: AlphaBlending::default(),
					source_node_id,
				});
			}
			table
		}
		EitherFormat::ArtboardGroupTable(artboard_group_table) => artboard_group_table,
	})
}

impl BoundingBox for Table<Artboard> {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		self.iter_ref().filter_map(|row| row.element.bounding_box(transform, include_stroke)).reduce(Quad::combine_bounds)
	}
}

#[node_macro::node(category(""))]
async fn to_artboard<Data: Into<Table<Graphic>> + 'n>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> DAffine2,
	)]
	contents: impl Node<Context<'static>, Output = Data>,
	label: String,
	location: DVec2,
	dimensions: DVec2,
	background: Color,
	clip: bool,
) -> Artboard {
	let location = location.as_ivec2();
	let dimensions = dimensions.as_ivec2().max(IVec2::ONE);

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
pub async fn append_artboard(_ctx: impl Ctx, mut artboards: Table<Artboard>, artboard: Artboard, node_path: Vec<NodeId>) -> Table<Artboard> {
	// Get the penultimate element of the node path, or None if the path is too short.
	// This is used to get the ID of the user-facing "Artboard" node (which encapsulates this internal "Append Artboard" node).
	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	artboards.push(TableRow {
		element: artboard,
		transform: DAffine2::IDENTITY,
		alpha_blending: AlphaBlending::default(),
		source_node_id: encapsulating_node_id,
	});

	artboards
}
