#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::view,
}

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif
#ifdef SRGB_OUTPUT
#import bevy_render::color_operations::linear_to_srgb
#endif
#ifdef OKLAB_OUTPUT
#import bevy_render::color_operations::linear_rgb_to_oklab
#endif

struct Lit {
    light: vec4<f32>,
};

struct Response {
    response: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> lit: Lit;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var albedo: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var albedo_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var relief: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var relief_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var emissive: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var emissive_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> material: Response;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(albedo, albedo_sampler, mesh.uv);
    let energy = material.response.x;
    var local = textureSample(relief, relief_sampler, mesh.uv).xyz * 2.0 - 1.0;
    local += vec3(
        sin(mesh.uv.y * 31.4159),
        cos(mesh.uv.x * 31.4159),
        0.0,
    ) * energy * 0.12;
    let t = normalize(mesh.world_tangent.xy);
    let normal = normalize(vec3(t.x * local.x - t.y * local.y, t.y * local.x + t.x * local.y, local.z));
    let direction = lit.light.xyz;
    let ambient = lit.light.w;
    let shade = (ambient + (1.0 - ambient) * max(dot(normal, direction), 0.0))
        / (ambient + (1.0 - ambient) * direction.z);
    let warm = mix(base.rgb, material.response.yzw, energy * 0.18);
    let flare = pow(max(dot(normal, direction), 0.0), 12.0) * energy * 0.45;
    let glow = textureSample(emissive, emissive_sampler, mesh.uv).r * energy;
    var output_color = vec4(warm * shade + warm * glow * 0.7 + flare, base.a);

#ifdef TONEMAP_IN_SHADER
    output_color = tonemapping::tone_mapping(output_color, view.color_grading);
#endif
#ifdef SRGB_OUTPUT
    output_color = vec4(linear_to_srgb(output_color.rgb), output_color.a);
#endif
#ifdef OKLAB_OUTPUT
    output_color = vec4(linear_rgb_to_oklab(output_color.rgb), output_color.a);
#endif
    return output_color;
}
