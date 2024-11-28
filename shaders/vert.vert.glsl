#version 450

layout(location = 0) out vec2 fragCoord;

void main() {
    // Full-screen quad vertices
    vec2 positions[6] = vec2[](
        vec2(-1.0, -1.0), // Bottom-left
        vec2(1.0, -1.0),  // Bottom-right
        vec2(-1.0, 1.0),  // Top-left
        vec2(-1.0, 1.0),  // Top-left
        vec2(1.0, -1.0),  // Bottom-right
        vec2(1.0, 1.0)    // Top-right
    );

    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragCoord = positions[gl_VertexIndex];
}
