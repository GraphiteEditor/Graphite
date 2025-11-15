use crate::graphic::Graphic;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, IVec2};
use core_types::blending::AlphaBlending;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::math::quad::Quad;
use core_types::render_complexity::RenderComplexity;
use core_types::table::{Table, TableRow};
use core_types::transform::Transform;
use core_types::uuid::NodeId;
use core_types::Color;
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

impl RenderComplexity for Artboard {
	fn render_complexity(&self) -> usize {
		self.content.render_complexity()
	}
}

// Implementations for Artboard
impl Transform for Artboard {
	fn transform(&self) -> DAffine2 {
		DAffine2::from_translation(self.location.as_dvec2())
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.location.as_dvec2() + self.dimensions.as_dvec2() * pivot
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

// Node definitions moved to graphic-nodes crate
