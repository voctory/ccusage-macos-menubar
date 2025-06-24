#!/bin/bash

# Icon generator script for ccusage menubar app
# Usage: ./generate-icon.sh [type] [size]
# Types: sparkle (default), chart, pulse, dots, lightning

TYPE="${1:-sparkle}"
SIZE="${2:-32}"
OUTPUT_DIR="src-tauri/icons"

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

case "$TYPE" in
    "sparkle")
        # Create a sparkle/star icon using drawn shapes
        CENTER=$((SIZE / 2))
        RADIUS=$((SIZE * 3 / 8))
        INNER_RADIUS=$((SIZE / 8))
        
        convert -size ${SIZE}x${SIZE} xc:transparent \
            -fill white \
            -draw "path 'M $CENTER,$((CENTER - RADIUS)) 
                   L $((CENTER + INNER_RADIUS)),$((CENTER - INNER_RADIUS)) 
                   L $((CENTER + RADIUS)),$CENTER 
                   L $((CENTER + INNER_RADIUS)),$((CENTER + INNER_RADIUS)) 
                   L $CENTER,$((CENTER + RADIUS)) 
                   L $((CENTER - INNER_RADIUS)),$((CENTER + INNER_RADIUS)) 
                   L $((CENTER - RADIUS)),$CENTER 
                   L $((CENTER - INNER_RADIUS)),$((CENTER - INNER_RADIUS)) 
                   Z'" \
            "$OUTPUT_DIR/sparkle.png"
        echo "Created sparkle icon at $OUTPUT_DIR/sparkle.png"
        ;;
    
    "chart")
        # Create a simple bar chart icon
        BAR_WIDTH=$((SIZE / 5))
        BAR_SPACING=$((SIZE / 10))
        START_X=$((SIZE / 5))
        
        convert -size ${SIZE}x${SIZE} xc:transparent \
            -fill white \
            -draw "rectangle $START_X,$((SIZE * 3 / 4)) $((START_X + BAR_WIDTH)),$SIZE" \
            -draw "rectangle $((START_X + BAR_WIDTH + BAR_SPACING)),$((SIZE / 2)) $((START_X + 2 * BAR_WIDTH + BAR_SPACING)),$SIZE" \
            -draw "rectangle $((START_X + 2 * (BAR_WIDTH + BAR_SPACING))),$((SIZE / 4)) $((START_X + 3 * BAR_WIDTH + 2 * BAR_SPACING)),$SIZE" \
            "$OUTPUT_DIR/chart.png"
        echo "Created chart icon at $OUTPUT_DIR/chart.png"
        ;;
    
    "pulse")
        # Create a pulse/wave icon
        convert -size ${SIZE}x${SIZE} xc:transparent \
            -stroke white -strokewidth 2 -fill none \
            -draw "path 'M 0,$((SIZE/2)) Q $((SIZE/4)),$((SIZE/3)) $((SIZE/2)),$((SIZE/2)) T $SIZE,$((SIZE/2))'" \
            "$OUTPUT_DIR/pulse.png"
        echo "Created pulse icon at $OUTPUT_DIR/pulse.png"
        ;;
    
    "dots")
        # Create three dots icon
        DOT_SIZE=$((SIZE / 8))
        CENTER_Y=$((SIZE / 2))
        SPACING=$((SIZE / 3))
        
        convert -size ${SIZE}x${SIZE} xc:transparent \
            -fill white \
            -draw "circle $((SIZE/6)),$CENTER_Y $((SIZE/6 + DOT_SIZE)),$CENTER_Y" \
            -draw "circle $((SIZE/2)),$CENTER_Y $((SIZE/2 + DOT_SIZE)),$CENTER_Y" \
            -draw "circle $((SIZE * 5/6)),$CENTER_Y $((SIZE * 5/6 + DOT_SIZE)),$CENTER_Y" \
            "$OUTPUT_DIR/dots.png"
        echo "Created dots icon at $OUTPUT_DIR/dots.png"
        ;;
    
    "lightning")
        # Create a lightning bolt icon
        convert -size ${SIZE}x${SIZE} xc:transparent \
            -fill white -gravity center \
            -font Arial-Bold -pointsize $((SIZE * 3 / 4)) \
            -annotate +0+0 "⚡" \
            "$OUTPUT_DIR/lightning.png"
        echo "Created lightning icon at $OUTPUT_DIR/lightning.png"
        ;;
    
    *)
        echo "Unknown icon type: $TYPE"
        echo "Available types: sparkle, chart, pulse, dots, lightning"
        exit 1
        ;;
esac

# Also generate @2x version for retina displays
RETINA_SIZE=$((SIZE * 2))
case "$TYPE" in
    "sparkle")
        convert -size ${RETINA_SIZE}x${RETINA_SIZE} xc:transparent \
            -fill white -gravity center \
            -font Arial-Bold -pointsize $((RETINA_SIZE * 3 / 4)) \
            -annotate +0+0 "✦" \
            "$OUTPUT_DIR/sparkle@2x.png"
        ;;
    "lightning")
        convert -size ${RETINA_SIZE}x${RETINA_SIZE} xc:transparent \
            -fill white -gravity center \
            -font Arial-Bold -pointsize $((RETINA_SIZE * 3 / 4)) \
            -annotate +0+0 "⚡" \
            "$OUTPUT_DIR/lightning@2x.png"
        ;;
esac

echo "Icon generation complete!"