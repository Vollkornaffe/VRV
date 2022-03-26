#version 450

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;

layout(location = 0) out vec3 fragColor;

void main() {
    fragColor = inColor;
    gl_Position = vec4(inPosition, 1.0);
    //float x = float(1 - int(gl_VertexIndex)) * 0.5;
    //float y = float(int(gl_VertexIndex & 1) * 2 - 1) * 0.5;
    //gl_Position = vec4(x, y, 0.0, 1.0);
    //fragColor = vec3(1.0, 1.0, 1.0);
}