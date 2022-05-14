#version 450

layout(binding = 0) uniform UBO {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inUV;
layout(location = 3) in vec4 inColor;

layout(location = 0) out vec3 outPosition;
layout(location = 1) out vec3 outNormal;
layout(location = 2) out vec2 outUV;
layout(location = 3) out vec4 outColor;

void main() {

    outPosition = (ubo.model * vec4(inPosition, 1.0)).xyz;
    outNormal = (transpose(inverse(ubo.model)) * vec4(inNormal, 0.0)).xyz;
    outUV = inUV;
    outColor = inColor;

    gl_Position = ubo.proj * ubo.view * vec4(outPosition, 1.0);
}