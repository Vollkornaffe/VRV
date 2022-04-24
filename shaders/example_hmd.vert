#version 450

#extension GL_EXT_multiview : require

layout(binding = 0) uniform UBO {
    mat4 model;
    mat4 view_left;
    mat4 view_right;
    mat4 proj_left;
    mat4 proj_right;
} ubo;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;

layout(location = 0) out vec3 fragColor;

void main() {
    fragColor = inColor;
    gl_Position = 
        (gl_ViewIndex == 0 ? ubo.proj_left : ubo.proj_right) *
        (gl_ViewIndex == 0 ? ubo.view_left : ubo.view_right) *
        ubo.model *
        vec4(inPosition, 1.0);
}