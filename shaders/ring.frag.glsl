#version 450
#extension GL_EXT_debug_printf : enable

layout(location = 0) in vec2 fragCoord; // NDC coordinates ranging from -1 to 1
layout(location = 0) out vec4 outColor;

layout(binding = 0) uniform Uniforms {
    float radius;       // Outer radius of the menu
} ubo;

void main() {
    float dist = length(fragCoord);
    float inner_radius = 0.02; // Inner radius of the cutout
    // Discard pixels outside the ring
    if (dist < inner_radius || dist > ubo.radius) {
        discard;
    }
    
    // Set color for the ring
    outColor = vec4(1.0, 1.0, 1.0, 1.0); // White ring
}
