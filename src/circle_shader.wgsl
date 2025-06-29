struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) circle_index: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) circle_index: u32,
    @location(1) uv: vec2<f32>,
};

struct Camera {
    position: vec2<f32>,
    vertical_height: f32,
    aspect: f32,
};

@group(0)
@binding(0)
var<uniform> camera: Camera;

struct Circle {
    position: vec3<f32>,
    color: vec3<f32>,
    radius: f32,
};

@group(1)
@binding(0)
var<storage, read> circles: array<Circle>;

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.circle_index = input.circle_index;

    output.uv = vec2<f32>(
        f32((input.vertex_index >> 0u) & 1u) * 2.0 - 1.0,
        f32((input.vertex_index >> 1u) & 1u) * 2.0 - 1.0,
    );

    let circle = circles[input.circle_index];
    let world_position = output.uv * circle.radius + circle.position.xy;

    output.clip_position = vec4<f32>(2.0 * (world_position - camera.position) / (camera.vertical_height * vec2<f32>(camera.aspect, 1.0)), 1.0 - circle.position.z, 1.0);

    return output;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    if dot(input.uv, input.uv) > 1.0 {
        discard;
    }
    return vec4<f32>(circles[input.circle_index].color, 1.0);
}
