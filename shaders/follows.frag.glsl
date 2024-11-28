#version 450
#extension GL_EXT_debug_printf : enable

layout(location = 0) in vec2 fragCoord;
layout(location = 0) out vec4 outColor;

layout(binding = 0) uniform Uniforms {
    float radius;
    int segments;
    float time;
    vec2 mouse_pos;
} ubo;

void main() {

    float inner_radius = 0.05; // Inner radius of the cutout
    float segment_gap = 0.1;   // Gap between segments in radians
    
    // Invert Y-axis if necessary (depending on your coordinate system)
    vec2 adjustedFragCoord = vec2(fragCoord.x, -fragCoord.y);

    // Normalize coordinates relative to the mouse position
    vec2 coord = adjustedFragCoord - ubo.mouse_pos;

    // Calculate distance from the center (mouse position)
    float dist = length(coord);

    // Discard the pixel if it falls inside the inner radius (cutout)
    if (dist < inner_radius || dist > ubo.radius) {
        discard;
    }

    // Calculate angle from the center to the pixel
    float angle = atan(coord.y, coord.x);
    if (angle < 0.0) {
        angle += 2.0 * 3.14159265359; // Convert negative angles to positive
    }

    // Calculate the angular width of each segment and account for the gap
    float segmentAngle = (2.0 * 3.14159265359 / float(ubo.segments)) - segment_gap;

    // Calculate the index of the current segment
    int segmentIndex = int(angle / (segmentAngle + segment_gap));

    // Check if the pixel falls within the current segment (exclude the gap)
    float segmentStartAngle = segmentIndex * (segmentAngle + segment_gap);
    float segmentEndAngle = segmentStartAngle + segmentAngle;
    if (angle < segmentStartAngle || angle > segmentEndAngle) {
        discard;
    }

    // Pulsing effect for the selected slice (when mouse is over it)
    //bool isMouseOver = (angle >= segmentStartAngle && angle <= segmentEndAngle && dist <= ubo.radius && dist >= inner_radius);
    float pulsingRadius = ubo.radius;
    //if (isMouseOver) {
    //    pulsingRadius += 0.1 * sin(ubo.time * 2.0); // Pulse effect only for the hovered segment
    //}

    // Discard pixel if outside the pulsing segment radius
    if (dist > pulsingRadius) {
        discard;
    }

    // Set color for the current slice
    outColor = vec4(1.0, 1.0, 1.0, 1.0); // White for the active segments
}
