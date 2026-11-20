//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

// Version 2 is where old Mix nodes join the lineage, and stored Mix Colors nodes already have its layout
rename!("raster_nodes::blending_nodes::MixColorsNode"@1 => @2);
