#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_norm;
layout(location = 2) in vec4 a_tang;
layout(location = 3) in vec2 a_uv;
// vec4[4] is used instead of mat4 due to spirv-cross bug for dx12 backend
layout(location = 4) in vec4 model[4]; // per-instance.

layout(std140, set = 1, binding = 0) uniform Args {
    mat4 proj;
    mat4 view;
    vec3 camera_pos;
};

layout(location = 0) out vec4 frag_world_pos;
layout(location = 1) out vec3 frag_norm;
layout(location = 2) out vec3 frag_tang;
layout(location = 3) flat out float frag_tbn_handedness;
layout(location = 4) out vec2 frag_uv;

void main() {
    mat4 model_mat = mat4(model[0], model[1], model[2], model[3]);
    frag_uv = a_uv;
    frag_norm = normalize((model_mat * vec4(a_norm, 0.0)).xyz);
    frag_tang = normalize((model_mat * vec4(a_tang.xyz, 0.0)).xyz);
    frag_tbn_handedness = a_tang.w;
    frag_world_pos = model_mat * vec4(a_pos, 1.0);
    gl_Position = proj * view * frag_world_pos;
}