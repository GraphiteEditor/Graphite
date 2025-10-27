#!/usr/bin/env bash

# Build the visualization tool if it doesn't exist
if [ ! -f "tools/crate-hierarchy-viz/target/debug/crate-hierarchy-viz" ]; then
    echo "Building crate hierarchy visualization tool..."
    cargo build
fi

# Generate the DOT file
echo "Generating crate hierarchy graph..."
./target/debug/crate-hierarchy-viz --workspace ../.. --format dot --output crate-hierarchy.dot --exclude-dyn-any

echo "Generating crate hierarchy graph (excluding dyn-any)..."
./target/debug/crate-hierarchy-viz --workspace ../.. --format dot --exclude-dyn-any --output crate-hierarchy-no-dyn-any.dot

# Generate visualizations if graphviz is available
if command -v dot &> /dev/null; then
    echo "Generating PNG visualizations..."
    dot -Tpng crate-hierarchy.dot -o crate-hierarchy.png
    dot -Tpng crate-hierarchy-no-dyn-any.dot -o crate-hierarchy-no-dyn-any.png

    echo "Generating SVG visualizations..."
    dot -Tsvg crate-hierarchy.dot -o crate-hierarchy.svg
    dot -Tsvg crate-hierarchy-no-dyn-any.dot -o crate-hierarchy-no-dyn-any.svg

    echo "Visualizations generated:"
    echo "  - crate-hierarchy.dot (GraphViz DOT format)"
    echo "  - crate-hierarchy.png (PNG image)"
    echo "  - crate-hierarchy.svg (SVG image)"
    echo "  - crate-hierarchy-no-dyn-any.dot (GraphViz DOT format, dyn-any excluded)"
    echo "  - crate-hierarchy-no-dyn-any.png (PNG image, dyn-any excluded)"
    echo "  - crate-hierarchy-no-dyn-any.svg (SVG image, dyn-any excluded)"
else
    echo "GraphViz not found. Generated DOT file only:"
    echo "  - crate-hierarchy.dot"
    echo "Install GraphViz to generate PNG/SVG visualizations"
fi
