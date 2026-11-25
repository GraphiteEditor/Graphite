//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

rename!("vector_nodes::vector_modification_nodes::PathModifyNode"@1 => "vector_nodes::vector_modification_nodes::PathNode"@2);
// Version 2 only adds `InjectFootprint` to the context bounds, so stored nodes keep their layout
rename!("core_types::vector::AreaNode"@1 => @2);
rename!("core_types::vector::CentroidNode"@1 => @2);
// Version 2 only lets its output carry an error, so stored nodes keep their layout
rename!("core_types::vector::ScatterPointsNode"@1 => @2);
