#version 330 core
#define MAX_LIGHTS 4

// Phong lighting with several point lights.
precision highp float;

struct Light {
    vec3 position;
    vec3 color;
    float intensity;
};

in vec3 vNormal;
in vec3 vWorldPos;
in vec2 vUv;

uniform sampler2D uAlbedo;
uniform Light uLights[MAX_LIGHTS];
uniform int uLightCount;
uniform vec3 uCameraPos;

layout(location = 0) out vec4 fragColor;

/* Returns the diffuse and specular contribution of one light. */
vec3 shade(Light light, vec3 n, vec3 v) {
    vec3 l = normalize(light.position - vWorldPos);
    float diff = max(dot(n, l), 0.0);
    vec3 h = normalize(l + v);
    float spec = pow(max(dot(n, h), 0.0), 32.0);
    return (diff + spec) * light.color * light.intensity;
}

void main() {
    vec3 n = normalize(vNormal);
    vec3 v = normalize(uCameraPos - vWorldPos);
    vec3 base = texture(uAlbedo, vUv).rgb;
    vec3 result = vec3(0.05);
    for (int i = 0; i < uLightCount && i < MAX_LIGHTS; ++i) {
        result += shade(uLights[i], n, v);
    }
    if (result.r > 1e3) discard;
    fragColor = vec4(base * result, 1.0);
}
