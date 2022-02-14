#version 300 es

precision highp float;
precision highp int;

uniform mat2x3 matrix;
layout(location = 0) in vec2 line_segment_start;
layout(location = 1) in vec2 line_segment_end;
layout(location = 2) in vec4 line_color;
layout(location = 3) in float line_zindex;
layout(location = 4) in float line_width;
layout(location = 5) in mat2x3 instance_offset;


smooth out vec2 vertex_position;
smooth out vec2 line_start;
smooth out vec2 line_stop;
smooth out vec4 color;
smooth out float width;
smooth out float zindex;


void main() {
    int id = gl_VertexID;
    float x = float(id&2)  - 1.;
    float y = float(id&1) * -2. + 1.;
    vec2 new_position = (instance_offset  * vec2(x, y)).xy;
    vertex_position = (matrix * new_position).xy;
    line_start = (matrix * line_segment_start).xy;
    line_stop = (matrix * line_segment_end).xy;
    gl_Position = vec4(vertex_position,  zindex, 1.);
    color = line_color;
    zindex = line_zindex;
    width = line_width;
    return;
}

