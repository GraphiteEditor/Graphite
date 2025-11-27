#!/usr/bin/env bash

# Build the visualization tool first to explain the wait time
echo "Building crate hierarchy visualization tool..."
cargo build

# Generate the DOT file
echo "Generating crate hierarchy graph..."
cargo run -- --workspace ../.. --output crate-hierarchy.dot


# Generate visualizations if graphviz is available
if command -v dot &> /dev/null; then
    echo "Generating PNG visualizations..."
    dot -Tpng crate-hierarchy.dot -o crate-hierarchy.png

    echo "Generating SVG visualizations..."
    dot -Tsvg crate-hierarchy.dot -o crate-hierarchy.svg

    echo "Visualizations generated:"
    echo "  - crate-hierarchy.dot (GraphViz DOT format)"
    echo "  - crate-hierarchy.png (PNG image)"
    echo "  - crate-hierarchy.svg (SVG image)"
else
    echo "GraphViz not found. Generated DOT file only:"
    echo "  - crate-hierarchy.dot"
    echo "Install GraphViz to generate PNG/SVG visualizations"
fi
