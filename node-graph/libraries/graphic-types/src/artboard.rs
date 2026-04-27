use crate::graphic::Graphic;
use core_types::Color;
use core_types::blending::AlphaBlending;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::math::quad::Quad;
use core_types::render_complexity::RenderComplexity;
use core_types::table::{Table, TableRow};
use core_types::transform::Transform;
use core_types::uuid::NodeId;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, IVec2};
use graphene_hash::CacheHash;

/// Some [`ArtboardData`] with some optional clipping bounds that can be exported.
#[derive(Clone, Debug, CacheHash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
			clip: true,
		}
	}
}

impl BoundingBox for Artboard {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let artboard_bounds = || (transform * Quad::from_box([self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()])).bounding_box();

		if self.clip {
			return RenderBoundingBox::Rectangle(artboard_bounds());
		}

		let mut combined_bounds = None;

		for (element, row_transform) in self.content.iter_element_values().zip(self.content.iter_attribute_values_or_default::<DAffine2>("transform")) {
			match element.bounding_box(transform * row_transform, include_stroke) {
				RenderBoundingBox::None => continue,
				RenderBoundingBox::Infinite => return RenderBoundingBox::Infinite,
				RenderBoundingBox::Rectangle(bounds) => match combined_bounds {
					Some(existing) => combined_bounds = Some(Quad::combine_bounds(existing, bounds)),
					None => combined_bounds = Some(bounds),
				},
			}
		}

		match combined_bounds {
			Some(content_bounds) => RenderBoundingBox::Rectangle(Quad::combine_bounds(content_bounds, artboard_bounds())),
			None => RenderBoundingBox::Rectangle(artboard_bounds()),
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

	#[derive(Clone, Default, Debug, PartialEq, DynAny)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct ArtboardGroup {
		pub artboards: Vec<(Artboard, Option<NodeId>)>,
	}

	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(untagged))]
	enum ArtboardFormat {
		ArtboardGroup(ArtboardGroup),
		OldArtboardTable(OldTable<Artboard>),
		ArtboardTable(Table<Artboard>),
	}

	#[derive(Clone, Debug)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct OldTable<T> {
		#[cfg_attr(feature = "serde", serde(alias = "instances", alias = "instance"))]
		element: Vec<T>,
		transform: Vec<DAffine2>,
		alpha_blending: Vec<AlphaBlending>,
	}

	Ok(match ArtboardFormat::deserialize(deserializer)? {
		ArtboardFormat::ArtboardGroup(artboard_group) => {
			let mut table = Table::new();
			for (artboard, source_node_id) in artboard_group.artboards {
				table.push(
					TableRow::new_from_element(artboard)
						.with_attribute("transform", DAffine2::IDENTITY)
						.with_attribute("alpha_blending", AlphaBlending::default())
						.with_attribute("source_node_id", source_node_id),
				);
			}
			table
		}
		ArtboardFormat::OldArtboardTable(old_table) => old_table
			.element
			.into_iter()
			.zip(old_table.transform.into_iter().zip(old_table.alpha_blending))
			.map(|(element, (transform, alpha_blending))| {
				TableRow::new_from_element(element)
					.with_attribute("transform", transform)
					.with_attribute("alpha_blending", alpha_blending)
					.with_attribute("source_node_id", None::<NodeId>)
			})
			.collect(),
		ArtboardFormat::ArtboardTable(artboard_table) => artboard_table,
	})
}

// Node definitions moved to graphic-nodes crate
