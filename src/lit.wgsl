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

struct Response {
    response: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(1) var albedo: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var albedo_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> material: Response;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(albedo, albedo_sampler, mesh.uv);
    let energy = material.response.x;
    let warm = mix(base.rgb, material.response.yzw, energy * 0.18);
    var output_color = vec4(warm * (1.0 + energy * 0.7), base.a);

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
