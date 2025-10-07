use crate::blending::AlphaBlending;
use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::gradient::GradientStops;
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
	pub content: Table<Graphic>,
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
			content: Table::new(),
			label: "Artboard".to_string(),
			location: location.min(location + dimensions),
			dimensions: dimensions.abs(),
			background: Color::WHITE,
			clip: false,
		}
	}
}

impl BoundingBox for Artboard {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let artboard_bounds = || (transform * Quad::from_box([self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()])).bounding_box();

		if self.clip {
			return RenderBoundingBox::Rectangle(artboard_bounds());
		}

		match self.content.bounding_box(transform, include_stroke) {
			RenderBoundingBox::Rectangle(content_bounds) => RenderBoundingBox::Rectangle(Quad::combine_bounds(content_bounds, artboard_bounds())),
			other => other,
		}
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_artboard<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Artboard>, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Default, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
	pub struct ArtboardGroup {
		pub artboards: Vec<(Artboard, Option<NodeId>)>,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum ArtboardFormat {
		ArtboardGroup(ArtboardGroup),
		OldArtboardTable(OldTable<Artboard>),
		ArtboardTable(Table<Artboard>),
	}

	#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
	pub struct OldTable<T> {
		#[serde(alias = "instances", alias = "instance")]
		element: Vec<T>,
		transform: Vec<DAffine2>,
		alpha_blending: Vec<AlphaBlending>,
	}

	Ok(match ArtboardFormat::deserialize(deserializer)? {
		ArtboardFormat::ArtboardGroup(artboard_group) => {
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
		ArtboardFormat::OldArtboardTable(old_table) => old_table
			.element
			.into_iter()
			.zip(old_table.transform.into_iter().zip(old_table.alpha_blending))
			.map(|(element, (transform, alpha_blending))| TableRow {
				element,
				transform,
				alpha_blending,
				source_node_id: None,
			})
			.collect(),
		ArtboardFormat::ArtboardTable(artboard_table) => artboard_table,
	})
}

/// Constructs a new single artboard table with the chosen properties.
#[node_macro::node(category(""))]
async fn create_artboard<T: Into<Table<Graphic>> + 'n>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
		Context -> DAffine2,
	)]
	content: impl Node<Context<'static>, Output = T>,
	label: String,
	location: DVec2,
	dimensions: DVec2,
	background: Table<Color>,
	clip: bool,
) -> Table<Artboard> {
	let location = location.as_ivec2();

	let footprint = ctx.try_footprint().copied();
	let mut new_ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.translate(location.as_dvec2());
		new_ctx = new_ctx.with_footprint(footprint);
	}
	let content = content.eval(new_ctx.into_context()).await.into();

	let dimensions = dimensions.as_ivec2().max(IVec2::ONE);

	let location = location.min(location + dimensions);

	let dimensions = dimensions.abs();

	let background: Option<Color> = background.into();
	let background = background.unwrap_or(Color::WHITE);

	Table::new_from_element(Artboard {
		content,
		label,
		location,
		dimensions,
		background,
		clip,
	})
}
