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
	use core_types::{
		AlphaBlending,
		table::{Table, TableRow},
	};
	use dyn_any::DynAny;
	use glam::DAffine2;
	use vector_types::vector::{PathStyle, PointDomain, RegionDomain, SegmentDomain, misc::HandleId};

	use crate::{Graphic, Vector};

	// TODO: Eventually remove this migration document upgrade code
	pub fn migrate_vector<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Vector>, D::Error> {
		use serde::Deserialize;

		#[derive(Clone, Debug, PartialEq, DynAny)]
		#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
		pub struct OldVectorData {
			pub transform: DAffine2,
			pub alpha_blending: AlphaBlending,

			pub style: PathStyle,

			pub colinear_manipulators: Vec<[HandleId; 2]>,

			pub point_domain: PointDomain,
			pub segment_domain: SegmentDomain,
			pub region_domain: RegionDomain,

			pub upstream_graphic_group: Option<Table<Graphic>>,
		}

		#[derive(Clone, Debug)]
		#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
		pub struct OldTable<T> {
			#[cfg_attr(feature = "serde", serde(alias = "instances", alias = "instance"))]
			element: Vec<T>,
			transform: Vec<DAffine2>,
			alpha_blending: Vec<AlphaBlending>,
		}

		#[derive(Clone, Debug)]
		#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
		pub struct OlderTable<T> {
			id: Vec<u64>,
			#[cfg_attr(feature = "serde", serde(alias = "instances", alias = "instance"))]
			element: Vec<T>,
		}

		#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
		#[cfg_attr(feature = "serde", serde(untagged))]
		#[allow(clippy::large_enum_variant)]
		enum VectorFormat {
			Vector(Vector),
			OldVectorData(OldVectorData),
			OldVectorTable(OldTable<Vector>),
			OlderVectorTable(OlderTable<Vector>),
			VectorTable(Table<Vector>),
		}

		Ok(match VectorFormat::deserialize(deserializer)? {
			VectorFormat::Vector(vector) => Table::new_from_element(vector),
			// Attributes (transform, alpha_blending, editor:layer) are not serialized, so migration only needs
			// to recover the elements. Per-row attribute values are populated at runtime by the node graph.
			VectorFormat::OldVectorData(old) => Table::new_from_element(Vector {
				style: old.style,
				colinear_manipulators: old.colinear_manipulators,
				point_domain: old.point_domain,
				segment_domain: old.segment_domain,
				region_domain: old.region_domain,
			}),
			VectorFormat::OlderVectorTable(older_table) => older_table.element.into_iter().map(TableRow::new_from_element).collect(),
			VectorFormat::OldVectorTable(old_table) => old_table.element.into_iter().map(TableRow::new_from_element).collect(),
			VectorFormat::VectorTable(vector_table) => vector_table,
		})
	}
}
