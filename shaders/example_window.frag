#version 450

layout(binding = 1) uniform sampler2D debugSampler;
layout(binding = 2) uniform sampler2D fontSampler;


layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inUV;
layout(location = 3) in vec3 inColor;

layout(location = 0) out vec4 outColor;

const vec3 debugLight = vec3(10.0, 10.0, 10.0);

void main() {

    vec3 toLight = debugLight - inPosition;
    float distToLight = length(toLight);

    float ambient = 0.3;
    float diffuse = clamp(dot(inNormal, toLight) /  distToLight, 0.0, 1.0);
    float light = clamp(ambient + diffuse, 0.0, 1.0);
    outColor = light * texture(debugSampler, inUV);
}