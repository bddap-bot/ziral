#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> opening: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> bite: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    if distance(mesh.uv, opening.xy) < opening.z && distance(mesh.uv, bite.xy) > bite.z {
        discard;
    }
    return vec4(0.0);
}
