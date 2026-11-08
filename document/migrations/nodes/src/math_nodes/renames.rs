//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

rename!("math_nodes::MathFxNode"@1 => "math_nodes::MathFunctionNode"@2);

// Version 2 takes lengths and angles too, with its output type following them, and stored nodes already have its layout
rename!("math_nodes::SineNode"@1 => @2);
rename!("math_nodes::CosineNode"@1 => @2);
rename!("math_nodes::TangentNode"@1 => @2);
rename!("math_nodes::SignNode"@1 => @2);
rename!("math_nodes::SumNode"@1 => @2);
rename!("math_nodes::AverageNode"@1 => @2);
rename!("math_nodes::MinimumNode"@1 => @2);
rename!("math_nodes::MaximumNode"@1 => @2);

// Version 2 reads a number beside a length or angle in pixels or degrees, and stored nodes already have its layout
rename!("math_nodes::LerpNode"@1 => @2);
rename!("math_nodes::ClampNode"@1 => @2);
rename!("math_nodes::LessThanNode"@1 => @2);
rename!("math_nodes::GreaterThanNode"@1 => @2);
rename!("math_nodes::EqualsNode"@1 => @2);
rename!("math_nodes::NotEqualsNode"@1 => @2);
