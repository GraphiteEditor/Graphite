pub mod adjustment;
pub mod adjustment_kernels;
pub mod adjustment_tables;
pub mod appearance;
pub mod artboard;
pub mod color_lookup_table;
pub mod effects;
pub mod generator;
pub mod graphic;
pub(crate) mod parameters;
pub mod raster_effects;
pub mod value_effects;

// Re-export all transitive dependencies so downstream crates only need to depend on graphic-types
pub use core_types;
pub use raster_types;
pub use vector_types;

// Re-export commonly used types at the crate root
pub use adjustment::Adjustment;
pub use appearance::{Appearance, Cover, CoverPlacement, Coverage, FillAndStroke, stamp_coverage};
pub use artboard::Artboard;
pub use effects::{BevelDirection, BevelStyle, Effect, EffectKind, EffectOperation, Effects, GlowSource, fitted_paint, stamp_effect};
pub use generator::Generator;
pub use graphic::{Graphic, IntoGraphicList, TryFromGraphic, Vector};
pub use raster_effects::evaluate_raster_effects;
pub use value_effects::{evaluate_color_effects, evaluate_gradient_effects};

pub mod migrations {
	use crate::Vector;
	use core_types::Color;
	use vector_types::gradient::GradientStops;
	use vector_types::{Gradient, GradientRamp, GradientSpace};

	// Storing legacy structs that are only used in document migration.
	// TODO: Eventually remove this document upgrade code
	pub mod legacy {
		use core_types::Color;
		use core_types::list::List;
		use dyn_any::DynAny;
		use glam::{DAffine2, DVec2};
		use vector_types::vector::misc::{BezierHandles, HandleId};
		use vector_types::vector::style::Stroke;
		use vector_types::vector::{Path, PointId, SegmentId, vector_from_legacy_parts};
		use vector_types::{GradientRamp, Vector, vector};

		#[derive(Default, Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny, serde::Serialize, serde::Deserialize)]
		pub struct LegacyGradient {
			#[serde(deserialize_with = "crate::migrations::migrate_to_gradient_ramp")]
			pub stops: GradientRamp,
			pub gradient_type: vector::style::GradientForm,
			pub start: DVec2,
			pub end: DVec2,
			#[serde(default)]
			pub spread_method: vector::style::GradientSpread,
			#[serde(default)]
			pub absolute: bool,
			#[serde(default)]
			pub transform: DAffine2,
		}

		impl LegacyGradient {
			/// Converts a legacy bounding-box-relative gradient (`start`/`end` in [0,1]) into an absolute one in the geometry's local space.
			/// `bounding_box` maps [0,1] onto the geometry's bounding box; `layer_transform` is the layer's own transform,
			/// used to bake the elliptical adjustment that reproduces the legacy isotropic radial through a non-uniform layer.
			pub fn to_absolute(&self, bounding_box: DAffine2, layer_transform: DAffine2) -> LegacyGradient {
				let start = bounding_box.transform_point2(self.start);
				let end = bounding_box.transform_point2(self.end);
				let direction = end - start;

				// The legacy radial drew as a circle in the layer's own space; bake the adjustment that, composed with the
				// endpoint frame, makes the new pipeline reproduce that circle through the (possibly non-uniform) layer transform.
				let radial_invertible = self.gradient_type == vector::style::GradientForm::Radial
					&& layer_transform.is_finite()
					&& layer_transform.matrix2.determinant().recip().is_finite()
					&& direction.length_squared() > 1e-20;
				let transform = if radial_invertible {
					let radius = (layer_transform.matrix2 * direction).length();
					let circle = DAffine2 {
						matrix2: glam::DMat2::from_diagonal(DVec2::splat(radius)),
						translation: layer_transform.transform_point2(start),
					};
					let base = DAffine2::from_cols(direction, direction.perp(), start);
					(layer_transform.inverse() * circle) * base.inverse()
				} else {
					DAffine2::IDENTITY
				};

				LegacyGradient {
					start,
					end,
					transform,
					absolute: true,
					..self.clone()
				}
			}

			/// Builds the affine that places the gradient endpoints at `start` and `end` when applied to canonical gradient space (0, 0) -> (1, 0).
			pub fn to_transform(&self) -> DAffine2 {
				let direction = self.end - self.start;
				DAffine2::from_cols(direction, direction.perp(), self.start)
			}
		}

		#[derive(Default, Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny, serde::Serialize, serde::Deserialize)]
		pub enum LegacyFill {
			#[default]
			None,
			Solid(Color),
			Gradient(LegacyGradient),
		}

		/// The legacy `fill` field is intentionally omitted because vector payload migration only
		/// recovers editable vector data. The stroke parses solely to validate the legacy shape.
		#[derive(serde::Deserialize)]
		#[cfg_attr(test, derive(Default, serde::Serialize))]
		pub(super) struct PathStyle {
			#[allow(dead_code)]
			pub stroke: Option<Stroke>,
		}

		/// The legacy point storage shape, kept only so old documents still deserialize.
		#[derive(Default, serde::Deserialize)]
		#[cfg_attr(test, derive(serde::Serialize))]
		pub(super) struct LegacyPointDomain {
			pub id: Vec<PointId>,
			#[serde(alias = "positions")]
			pub position: Vec<DVec2>,
		}

		/// The legacy segment storage shape, kept only so old documents still deserialize.
		#[derive(Default, serde::Deserialize)]
		#[cfg_attr(test, derive(serde::Serialize))]
		pub(super) struct LegacySegmentDomain {
			#[serde(alias = "ids")]
			pub id: Vec<SegmentId>,
			pub start_point: Vec<usize>,
			pub end_point: Vec<usize>,
			pub handles: Vec<BezierHandles>,
		}

		/// The legacy `Vector` struct shape, converted into pool storage at load.
		#[derive(Default, serde::Deserialize)]
		#[cfg_attr(test, derive(serde::Serialize))]
		pub(super) struct LegacyVector {
			pub colinear_manipulators: Vec<[HandleId; 2]>,
			pub point_domain: LegacyPointDomain,
			pub segment_domain: LegacySegmentDomain,
		}

		impl LegacyVector {
			pub(super) fn into_parts(self) -> (Vector, List<Path>, std::collections::HashSet<SegmentId>) {
				vector_from_legacy_parts(
					self.point_domain.id,
					self.point_domain.position,
					self.segment_domain.id,
					self.segment_domain.start_point,
					self.segment_domain.end_point,
					self.segment_domain.handles,
					self.colinear_manipulators,
				)
			}
		}

		/// Old documents stored a `Vector` flattened with list attributes (`transform`, `alpha_blending`, `upstream_graphic_group`); only the geometry fields are recovered.
		#[derive(serde::Deserialize)]
		#[cfg_attr(test, derive(Default, serde::Serialize))]
		pub(super) struct VectorData {
			#[allow(dead_code)]
			pub style: PathStyle,
			pub colinear_manipulators: Vec<[HandleId; 2]>,
			pub point_domain: LegacyPointDomain,
			pub segment_domain: LegacySegmentDomain,
		}

		#[derive(serde::Deserialize)]
		pub(super) struct Table {
			#[serde(alias = "instances", alias = "instance")]
			pub element: Vec<LegacyVector>,
		}
	}

	// TODO: Eventually remove this document upgrade code
	/// Returns the first vector recovered from any of the legacy on-disk shapes (the legacy `VectorData` flat struct, a legacy `Vector` struct, or any of the historical list variants).
	#[allow(clippy::type_complexity)]
	pub fn migrate_to_optional_vector<'de, D: serde::Deserializer<'de>>(
		deserializer: D,
	) -> Result<Option<(Vector, core_types::list::List<vector_types::vector::Path>, std::collections::HashSet<vector_types::vector::SegmentId>)>, D::Error> {
		use serde::Deserialize;

		#[derive(serde::Deserialize)]
		#[serde(untagged)]
		#[allow(clippy::large_enum_variant)]
		enum VectorFormat {
			// The old flat shape that also held a style. Both this and the legacy `Vector` arm recover the same geometry from it.
			OldVectorData(legacy::VectorData),
			Vector(legacy::LegacyVector),
			List(legacy::Table),
		}

		Ok(match VectorFormat::deserialize(deserializer)? {
			VectorFormat::OldVectorData(old) => {
				let vector = legacy::LegacyVector {
					colinear_manipulators: old.colinear_manipulators,
					point_domain: old.point_domain,
					segment_domain: old.segment_domain,
				};
				Some(vector.into_parts())
			}
			VectorFormat::Vector(vector) => Some(vector.into_parts()),
			VectorFormat::List(list) => list.element.into_iter().next().map(legacy::LegacyVector::into_parts),
		})
	}

	// TODO: Eventually remove this document upgrade code
	/// Recovers a [`GradientRamp`] from any of its on-disk shapes: the current nested form, the flat stops struct
	/// that preceded it, or the ancient position-color tuple list (whose even positions elide back to absence).
	/// The pre-ramp shapes come from documents that rendered in gamma, so they carry that space explicitly.
	pub fn migrate_to_gradient_ramp<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<GradientRamp, D::Error> {
		use serde::Deserialize;

		#[derive(serde::Deserialize)]
		#[serde(untagged)]
		enum GradientRampFormat {
			Ramp(GradientRamp),
			FlatStops(GradientStops<Color>),
			Tuples(Vec<(f64, Color)>),
		}

		Ok(match GradientRampFormat::deserialize(deserializer)? {
			GradientRampFormat::Ramp(ramp) => ramp,
			GradientRampFormat::FlatStops(stops) => GradientRamp {
				gradient_space: GradientSpace::RgbGamma,
				..GradientRamp::from(stops)
			},
			GradientRampFormat::Tuples(stops) => {
				let position: Vec<f64> = stops.iter().map(|(position, _)| *position).collect();
				let mut gradient = Gradient::from(stops.into_iter().map(|(_, color)| color).collect::<Vec<_>>());
				gradient.set_positions(&position);
				gradient.elide_default_attributes(false);

				GradientRamp {
					gradient_space: GradientSpace::RgbGamma,
					..GradientRamp::from(gradient)
				}
			}
		})
	}

	#[cfg(test)]
	mod migration_tests {
		use super::*;
		use vector_types::vector::style::Stroke;

		/// The legacy `style` payload is discarded, leaving only the geometry.
		#[test]
		fn recovers_geometry_from_old_vector_data_with_style() {
			let old_vector = legacy::VectorData {
				style: legacy::PathStyle { stroke: Some(Stroke::new(12.)) },
				point_domain: legacy::LegacyPointDomain {
					id: vec![vector_types::vector::PointId::ZERO],
					position: vec![glam::DVec2::new(3., 4.)],
				},
				..Default::default()
			};

			let mut value = serde_json::to_value(old_vector).unwrap();
			value
				.as_object_mut()
				.unwrap()
				.get_mut("style")
				.unwrap()
				.as_object_mut()
				.unwrap()
				.insert("fill".into(), serde_json::to_value(legacy::LegacyFill::default()).unwrap());

			let (migrated, _paths, _) = migrate_to_optional_vector(value).unwrap().expect("the legacy shape should parse into a vector");

			assert_eq!(
				migrated.positions().collect::<Vec<_>>(),
				[glam::DVec2::new(3., 4.)],
				"the legacy geometry should survive alongside the discarded style"
			);
		}

		#[test]
		fn recovers_geometry_from_legacy_vector_struct() {
			let vector = legacy::LegacyVector {
				point_domain: legacy::LegacyPointDomain {
					id: vec![vector_types::vector::PointId::ZERO],
					position: vec![glam::DVec2::new(3., 4.)],
				},
				..Default::default()
			};

			let value = serde_json::to_value(&vector).unwrap();
			let (migrated, _paths, _) = migrate_to_optional_vector(value).unwrap().unwrap();

			assert_eq!(migrated.positions().collect::<Vec<_>>(), [glam::DVec2::new(3., 4.)]);
		}

		/// The oldest documents named these fields in the plural.
		#[test]
		fn recovers_geometry_from_plural_field_names() {
			let value = serde_json::json!({
				"colinear_manipulators": [],
				"point_domain": { "id": [1, 2], "positions": [[3., 4.], [5., 6.]] },
				"segment_domain": { "ids": [7], "start_point": [0], "end_point": [1], "handles": ["Linear"] },
			});

			let (migrated, paths, _) = migrate_to_optional_vector(value).unwrap().expect("the plural field names should parse");

			assert_eq!(migrated.positions().collect::<Vec<_>>(), [glam::DVec2::new(3., 4.), glam::DVec2::new(5., 6.)]);
			assert_eq!(paths.len(), 1);
		}

		/// A closed triangle keeps its IDs, its handles, and the smooth pairs at its anchors, beside a free point.
		#[test]
		fn keeps_ids_handles_smooth_pairs_and_free_points() {
			use glam::DVec2;
			use vector_types::kurbo::{CubicBez, Line, PathSeg, QuadBez};
			use vector_types::vector::misc::HandleType;

			let handle = |ty: &str, segment: u64| serde_json::json!({ "ty": ty, "segment": segment });
			let value = serde_json::json!({
				"colinear_manipulators": [[handle("End", 101), handle("Primary", 102)], [handle("End", 103), handle("Primary", 101)]],
				"point_domain": { "id": [10, 20, 30, 40], "position": [[0., 0.], [10., 0.], [10., 10.], [99., 99.]] },
				"segment_domain": {
					"id": [101, 102, 103],
					"start_point": [0, 1, 2],
					"end_point": [1, 2, 0],
					"handles": [{ "Cubic": { "handle_start": [2., -1.], "handle_end": [8., -1.] } }, { "Quadratic": { "handle": [12., 5.] } }, "Linear"],
				},
			});

			let (vector, paths, _) = migrate_to_optional_vector(value).unwrap().expect("the legacy vector should parse");

			// Looked up by ID, since storage order is free to differ from the legacy order
			let mut anchors: Vec<(u64, DVec2)> = vector
				.anchor_points(&paths)
				.filter_map(|point| Some((point.inner(), vector.position(vector.index_of_point_id(point)? as usize)?)))
				.collect();
			anchors.sort_by_key(|&(id, _)| id);
			assert_eq!(anchors, [(10, DVec2::new(0., 0.)), (20, DVec2::new(10., 0.)), (30, DVec2::new(10., 10.)), (40, DVec2::new(99., 99.))]);

			let mut segments: Vec<(u64, PathSeg, u64, u64)> = vector.segment_iter(&paths).map(|(id, segment, start, end)| (id.inner(), segment, start.inner(), end.inner())).collect();
			segments.sort_by_key(|&(id, ..)| id);
			assert_eq!(
				segments,
				[
					(101, PathSeg::Cubic(CubicBez::new((0., 0.), (2., -1.), (8., -1.), (10., 0.))), 10, 20),
					(102, PathSeg::Quad(QuadBez::new((10., 0.), (12., 5.), (10., 10.))), 20, 30),
					(103, PathSeg::Line(Line::new((10., 10.), (0., 0.))), 30, 10),
				]
			);

			let mut smooth_pairs: Vec<[(bool, u64); 2]> = vector
				.colinear_pairs(&paths)
				.into_iter()
				.map(|pair| {
					let mut sides = pair.map(|handle| (handle.ty == HandleType::Primary, handle.segment.inner()));
					sides.sort();
					sides
				})
				.collect();
			smooth_pairs.sort();
			assert_eq!(smooth_pairs, [[(false, 101), (true, 102)], [(false, 103), (true, 101)]]);
		}
	}
}
