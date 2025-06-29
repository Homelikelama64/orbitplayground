struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) quad_index: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) quad_index: u32,
};

struct Camera {
    position: vec2<f32>,
    vertical_height: f32,
    aspect: f32,
};

@group(0)
@binding(0)
var<uniform> camera: Camera;

struct Quad {
    position: vec3<f32>,
    rotation: f32,
    color: vec3<f32>,
    size: vec2<f32>,
};

@group(1)
@binding(0)
var<storage, read> quads: array<Quad>;

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.quad_index = input.quad_index;

    let uv = vec2<f32>(
        f32((input.vertex_index >> 0u) & 1u) - 0.5,
        f32((input.vertex_index >> 1u) & 1u) - 0.5,
    );

    let quad = quads[input.quad_index];
    var world_position = uv * quad.size;
    world_position = vec2<f32>(
        world_position.x * sin(quad.rotation) - world_position.y * cos(quad.rotation),
        world_position.x * cos(quad.rotation) + world_position.y * sin(quad.rotation),
    );
    world_position += quad.position.xy;

    output.clip_position = vec4<f32>(2.0 * (world_position - camera.position) / (camera.vertical_height * vec2<f32>(camera.aspect, 1.0)), 1.0 - quad.position.z, 1.0);

    return output;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(quads[input.quad_index].color, 1.0);
}
