//! Nodes that only moved to a new identifier.

use migration_core::rename;

rename!("raster_nodes::filter::BlurNode"@1 => "blending_nodes::effects::BlurNode"@2);
