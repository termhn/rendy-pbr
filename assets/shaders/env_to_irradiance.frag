#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(constant_id = 0) const int THETA_SAMPLES = 256;

layout(location = 0) in vec3 f_pos;
layout(location = 1) flat in int face_index;

layout(set = 0, binding = 0) uniform sampler env_sampler;
layout(set = 0, binding = 1) uniform textureCube env_texture;

const float PI = 3.14159265359;

const int PHI_SAMPLES = THETA_SAMPLES/4;

layout(location = 0) out vec4 color;

void main() {
    vec3 pos = f_pos;
    vec3 N = normalize(pos);

    vec3 irradiance = vec3(0.0);

    vec3 up = vec3(0.0, 1.0, 0.0);
    vec3 right = cross(up, N);
    up = cross(N, right);
    

    float theta_sample_delta = 2.0 * PI / float(THETA_SAMPLES);
    float phi_sample_delta = 0.5 * PI / float(PHI_SAMPLES);

    float theta = 0.0;
    for (int theta_sample_count = 0; theta_sample_count < THETA_SAMPLES; theta_sample_count++) {
        float phi = 0.0;
        for (int phi_sample_count = 0; phi_sample_count < PHI_SAMPLES; phi_sample_count++) {
            // spherical to cartesian in tangent space
            vec3 tangent_sample = vec3(sin(phi) * cos(theta), sin(phi) * sin(theta), cos(phi));
            // tangent to world space
            vec3 sample_vec = tangent_sample.x * right + tangent_sample.y * up + tangent_sample.z * N;

            irradiance += texture(samplerCube(env_texture, env_sampler), sample_vec).rgb * cos(phi) * sin(phi);
            phi += phi_sample_delta;
        }
        theta += theta_sample_delta;
    }
    
    irradiance = PI * irradiance * (1.0 / float(THETA_SAMPLES * PHI_SAMPLES));

    color = vec4(irradiance, 1.0);
}
