pub mod artboard;
pub mod graphic;

// Re-export all transitive dependencies so downstream crates only need to depend on graphic-types
pub use core_types;
pub use raster_types;
pub use vector_types;

// Re-export commonly used types at the crate root
pub use artboard::Artboard;
pub use graphic::{Graphic, IntoGraphicList, TryFromGraphic, Vector};

pub mod migrations {
	use crate::Vector;
	use core_types::Color;
	use vector_types::gradient::GradientStops;
	use vector_types::{Gradient, GradientInterpolation, GradientRamp};

	// Storing legacy structs that are only used in document migration.
	// TODO: Eventually remove this document upgrade code
	pub mod legacy {
		use core_types::Color;
		use dyn_any::DynAny;
		use glam::{DAffine2, DVec2};
		use vector_types::vector::{PointDomain, RegionDomain, SegmentDomain, misc::HandleId, style::Stroke};
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
		/// recovers editable vector data. The fill/stroke paints are migrated from the node inputs.
		#[derive(serde::Deserialize)]
		#[cfg_attr(test, derive(Default, serde::Serialize))]
		pub(super) struct PathStyle {
			pub stroke: Option<Stroke>,
		}

		/// Old documents stored a `Vector` flattened with list attributes (`transform`, `alpha_blending`, `upstream_graphic_group`); only the geometry fields are recovered.
		#[derive(serde::Deserialize)]
		#[cfg_attr(test, derive(Default, serde::Serialize))]
		pub(super) struct VectorData {
			pub style: PathStyle,
			pub colinear_manipulators: Vec<[HandleId; 2]>,
			pub point_domain: PointDomain,
			pub segment_domain: SegmentDomain,
			pub region_domain: RegionDomain,
		}

		#[derive(serde::Deserialize)]
		pub(super) struct Table {
			#[serde(alias = "instances", alias = "instance")]
			pub element: Vec<Vector>,
		}
	}

	// TODO: Eventually remove this document upgrade code
	/// Returns the first `Vector` recovered from any of the legacy on-disk shapes (the legacy `VectorData` flat struct, a single `Vector`, or any of the historical `List<Vector>` variants).
	pub fn migrate_to_optional_vector<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Vector>, D::Error> {
		use serde::Deserialize;

		#[derive(serde::Deserialize)]
		#[serde(untagged)]
		#[allow(clippy::large_enum_variant)]
		enum VectorFormat {
			// Old vector data must be tried first. Serde would otherwise ignore its `style` field and
			// deserialize the missing optional `stroke` field as `None` in the current `Vector`.
			OldVectorData(legacy::VectorData),
			Vector(Vector),
			List(legacy::Table),
		}

		Ok(match VectorFormat::deserialize(deserializer)? {
			VectorFormat::OldVectorData(old) => Some(Vector {
				stroke: old.style.stroke,
				colinear_manipulators: old.colinear_manipulators,
				point_domain: old.point_domain,
				segment_domain: old.segment_domain,
				region_domain: old.region_domain,
			}),
			VectorFormat::Vector(vector) => Some(vector),
			VectorFormat::List(list) => list.element.into_iter().next(),
		})
	}

	// TODO: Eventually remove this document upgrade code
	/// Recovers a [`GradientRamp`] from any of its on-disk shapes: the current nested form, the flat stops struct
	/// that preceded it, or the ancient position-color tuple list (whose even positions elide back to absence).
	/// The pre-ramp shapes come from documents that rendered in gamma, so they carry that interpolation explicitly.
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
				gradient_interpolation: GradientInterpolation::RgbGamma,
				..GradientRamp::from(stops)
			},
			GradientRampFormat::Tuples(stops) => {
				let position: Vec<f64> = stops.iter().map(|(position, _)| *position).collect();
				let mut gradient = Gradient::from(stops.into_iter().map(|(_, color)| color).collect::<Vec<_>>());
				gradient.set_positions(&position);
				gradient.elide_default_attributes();

				GradientRamp {
					gradient_interpolation: GradientInterpolation::RgbGamma,
					..GradientRamp::from(gradient)
				}
			}
		})
	}

	#[cfg(test)]
	mod migration_tests {
		use super::*;
		use vector_types::vector::style::Stroke;

		#[test]
		fn preserves_stroke_from_old_vector_data_style() {
			let old_vector = legacy::VectorData {
				style: legacy::PathStyle { stroke: Some(Stroke::new(12.)) },
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
			let migrated = migrate_to_optional_vector(value).unwrap().unwrap();

			assert_eq!(migrated.stroke.unwrap().weight, 12.);
		}

		#[test]
		fn preserves_stroke_from_current_vector_data() {
			let vector = Vector {
				stroke: Some(Stroke::new(12.)),
				..Default::default()
			};

			let value = serde_json::to_value(&vector).unwrap();
			let migrated = migrate_to_optional_vector(value).unwrap().unwrap();

			assert_eq!(migrated.stroke.unwrap().weight, 12.);
		}
	}
}
