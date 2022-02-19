#version 300 es

precision highp float;
precision highp int;

uniform mat3x2 matrix;
layout(location = 0) in vec2 line_segment_start;
layout(location = 1) in vec2 line_segment_end;
layout(location = 2) in vec4 line_color;
layout(location = 3) in float line_zindex;
layout(location = 4) in float line_width;
layout(location = 5) in uint line_flags;
layout(location = 6) in mat3x2 instance_offset;


smooth out vec2 vertex_position;
flat out vec2 line_start;
flat out vec2 line_stop;
flat out vec4 color;
flat out float width;
flat out float zindex;

flat out uint flags;


void main() {
    int id = gl_VertexID;
    float x = float(id&2)  - 1.;
    float y = float(id&1) * -2. + 1.;
    vec2 new_position = instance_offset  * vec3(x, y, 1);
    vertex_position = matrix * vec3(new_position, 1);
    line_start = matrix * vec3(line_segment_start, 1);
    line_stop = matrix * vec3(line_segment_end, 1);
    gl_Position = vec4(vertex_position,  zindex, 1.);
    color = line_color;
    zindex = line_zindex;
    width = line_width;
    flags = line_flags;
}

