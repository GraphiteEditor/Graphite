pub mod artboard;
pub mod graphic;

// Re-export all transitive dependencies so downstream crates only need to depend on graphic-types
pub use core_types;
pub use raster_types;
pub use vector_types;

// Re-export commonly used types at the crate root
pub use artboard::Artboard;
pub use graphic::{Graphic, IntoGraphicTable, TryFromGraphic, Vector};

pub mod migrations {
	use vector_types::vector::{PathStyle, PointDomain, RegionDomain, SegmentDomain, misc::HandleId};

	use crate::Vector;

	// TODO: Eventually remove this migration document upgrade code
	/// Returns the first `Vector` recovered from any of the legacy on-disk shapes (a single `Vector`, the old `OldVectorData` flat struct, or any of the historical `Table<Vector>` variants).
	pub fn migrate_to_optional_vector<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Vector>, D::Error> {
		use serde::Deserialize;

		/// Old documents stored a `Vector` flattened with table attributes (`transform`, `alpha_blending`, `upstream_graphic_group`); only the geometry fields are recovered.
		#[derive(serde::Deserialize)]
		struct OldVectorData {
			style: PathStyle,
			colinear_manipulators: Vec<[HandleId; 2]>,
			point_domain: PointDomain,
			segment_domain: SegmentDomain,
			region_domain: RegionDomain,
		}

		#[derive(serde::Deserialize)]
		struct LegacyTable {
			#[serde(alias = "instances", alias = "instance")]
			element: Vec<Vector>,
		}

		#[derive(serde::Deserialize)]
		#[serde(untagged)]
		#[allow(clippy::large_enum_variant)]
		enum VectorFormat {
			Vector(Vector),
			OldVectorData(OldVectorData),
			Table(LegacyTable),
		}

		Ok(match VectorFormat::deserialize(deserializer)? {
			VectorFormat::Vector(vector) => Some(vector),
			VectorFormat::OldVectorData(old) => Some(Vector {
				style: old.style,
				colinear_manipulators: old.colinear_manipulators,
				point_domain: old.point_domain,
				segment_domain: old.segment_domain,
				region_domain: old.region_domain,
			}),
			VectorFormat::Table(table) => table.element.into_iter().next(),
		})
	}
}
