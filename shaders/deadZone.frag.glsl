#version 450
#extension GL_EXT_debug_printf : enable

layout(location = 0) in vec2 fragCoord; // NDC coordinates ranging from -1 to 1
layout(location = 0) out vec4 outColor;

layout(binding = 0) uniform Uniforms {
    float radius;       // Outer radius of the menu
    float inner_radius;
    int segments;       // Number of segments
    float time;         // Time for animations
    vec2 mouse_pos;     // Mouse position in NDC
    float segment_gap;
    int item_selected;
} ubo;

void main() {

    float inner_radius = 0.02; // Inner radius of the cutout
    float segment_gap = 0.1;  // Gap between segments in radians

    // Step 1: Set the center of the menu (fixed at origin)
    vec2 menu_center = vec2(0.0, 0.0); // Center of the window

    // Step 2: Calculate coordinates relative to the menu center
    vec2 coord = fragCoord - menu_center;

    // Optional: Invert Y-axis if your coordinate system requires it
    coord.y = -coord.y;

    // Step 3: Calculate distance from the center
    float dist = length(coord);

    // Step 4: Discard pixels outside the ring
    if (dist < ubo.inner_radius || dist > ubo.radius) {
        discard;
    }

    // Step 5: Calculate angle from the center to the current pixel
    float angle = atan(coord.y, coord.x);
    if (angle < 0.0) {
        angle += 2.0 * 3.14159265359; // Normalize angle to [0, 2π]
    }

    // Step 6: Calculate the total angle per segment including gaps
    float segmentAngleWithGap = (2.0 * 3.14159265359) / float(ubo.segments);
    float segmentAngle = segmentAngleWithGap - ubo.segment_gap; // Angular width of a segment

    // Step 7: Calculate the index of the current segment
    int segmentIndex = int(angle / segmentAngleWithGap);

    // Step 8: Determine the start and end angle of the current segment
    float segmentStartAngle = float(segmentIndex) * segmentAngleWithGap;
    float segmentEndAngle = segmentStartAngle + segmentAngle;

    // Step 9: Discard pixels that fall into the gap between segments
    if (angle < segmentStartAngle || angle > segmentEndAngle) {
        discard;
    }

    // Step 10: Calculate mouse position relative to the menu center
    vec2 mouse_coord = ubo.mouse_pos - menu_center;

    // Optional: Invert Y-axis for mouse position if necessary
    // mouse_coord.y = -mouse_coord.y;

    // Step 11: Calculate angle of the mouse position
    float mouse_angle = atan(mouse_coord.y, mouse_coord.x);
    if (mouse_angle < 0.0) {
        mouse_angle += 2.0 * 3.14159265359;
    }

    // Step 12: Only calculate hoverSegmentIndex if mouse is outside inner_radius
    int hoverSegmentIndex = -1;
    if (length(mouse_coord) > ubo.inner_radius) {
        float mouse_angle = atan(mouse_coord.y, mouse_coord.x);
        if (mouse_angle < 0.0) {
            mouse_angle += 2.0 * 3.14159265359;
        }
        hoverSegmentIndex = int(mouse_angle / segmentAngleWithGap);
    }

    // Step 13: Apply pulsing effect to the hovered segment
    float pulsingRadius = ubo.radius;
    if (segmentIndex == hoverSegmentIndex) {
        pulsingRadius += 0.05 * sin(ubo.time * 2.0); // Adjust pulse amplitude as needed
    }

    // Step 14: Discard pixel if it's outside the pulsing segment radius
    if (dist > pulsingRadius) {
        discard;
    }

    // Step 15: Set color for the current pixel
    // For debugging, assign different colors to different segments
    vec3 segmentColor = vec3(float(segmentIndex) / float(ubo.segments), 1.0, 1.0);
    if (segmentIndex == hoverSegmentIndex) {
        // Highlight the hovered segment
        outColor = vec4(segmentColor, 1.0);
    } else {
        outColor = vec4(segmentColor * 0.5, 1.0); // Dim other segments
    }
}
