# path_bool

A Rust library for performing boolean operations on SVG paths.

`path_bool` is a port of [PathBool.js](https://github.com/r-flash/PathBool.js), providing low-level functionality for boolean operations on complex 2D paths. It handles paths with multiple subpaths, self-intersections, and different fill rules.

## Features

- Supports multiple boolean operations: Union, Intersection, Difference, Exclusion, Division, and Fracture.
- Handles both `NonZero` and `EvenOdd` fill rules.
- Works with paths containing lines, cubic Bézier curves, quadratic Bézier curves, and elliptical arcs.
- Provides utilities for parsing and generating SVG path data.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
path_bool = "0.1.0"
```

## Usage

Here's a basic example of performing an intersection operation on two paths:

```rust
use path_bool::{path_boolean, FillRule, PathBooleanOperation, path_from_path_data, path_to_path_data};

fn main() {
    let path_a = path_from_path_data("M 10 10 L 50 10 L 30 40 Z");
    let path_b = path_from_path_data("M 20 30 L 60 30 L 60 50 L 20 50 Z");

    let result = path_boolean(
        &path_a,
        FillRule::NonZero,
        &path_b,
        FillRule::NonZero,
        PathBooleanOperation::Intersection
    ).unwrap();

    let result_data = path_to_path_data(&result[0], 0.001);
    println!("Result: {}", result_data);
}
```

## Algorithm

The boolean operations are implemented using a graph-based approach. After the parsing the input, self intersecting cubic beziers curves are simplified. Then the intersection points between all edges are calculated. These are then turned into a graph representation where every intersection becomes a new vertex. We then apply edge contractions to remove vertices with a degree of 2 to compute the graph minor. At this stage, identiacal edges are deduplicated. Because we are ultimately interrested in the faces of the graph to decide if they should be included in the final output, we then compute the dual graph in which the faces become vertices and vertices become the new faces. That dual structure is then used to determine which faces (dual vertices) should be included in the final output. 

## Development Status

This project is a port of PathBool.js and is still in early stages of development. Contributions, bug reports, and feedback are welcome!

## TODO

- Comprehensive test suite
- Performance optimizations
- Additional examples and documentation

## License

MIT License or Apache-2.0

Copyright © 2024 [ Graphite Authors ]

## Acknowledgements

This library is a Rust port of [PathBool.js](https://github.com/r-flash/PathBool.js) by Adam Platkevič.
