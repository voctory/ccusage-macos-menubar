#!/bin/bash

# Simple icon generator using basic shapes
SIZE="${1:-32}"
OUTPUT_DIR="src-tauri/icons"

mkdir -p "$OUTPUT_DIR"

# Create a simple filled circle as a test
magick -size ${SIZE}x${SIZE} xc:transparent \
    -fill white \
    -draw "circle $((SIZE/2)),$((SIZE/2)) $((SIZE/2)),$((SIZE/4))" \
    "$OUTPUT_DIR/circle.png"

echo "Created circle.png"

# Create a simple star using polygon
CENTER=$((SIZE / 2))
magick -size ${SIZE}x${SIZE} xc:transparent \
    -fill white \
    -draw "polygon $CENTER,$((SIZE/8)) $((SIZE*5/8)),$((SIZE*5/12)) $((SIZE*7/8)),$((SIZE/2)) $((SIZE*5/8)),$((SIZE*7/12)) $CENTER,$((SIZE*7/8)) $((SIZE*3/8)),$((SIZE*7/12)) $((SIZE/8)),$((SIZE/2)) $((SIZE*3/8)),$((SIZE*5/12))" \
    "$OUTPUT_DIR/star.png"

echo "Created star.png"

# Create simple bars
magick -size ${SIZE}x${SIZE} xc:transparent \
    -fill white \
    -draw "rectangle $((SIZE/6)),$((SIZE*2/3)) $((SIZE/3)),$((SIZE-2))" \
    -draw "rectangle $((SIZE*5/12)),$((SIZE/2)) $((SIZE*7/12)),$((SIZE-2))" \
    -draw "rectangle $((SIZE*2/3)),$((SIZE/3)) $((SIZE*5/6)),$((SIZE-2))" \
    "$OUTPUT_DIR/bars.png"

echo "Created bars.png"