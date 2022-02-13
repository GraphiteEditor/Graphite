#version 300 es

precision highp float;
precision highp int;

uniform mat3 matrix;

layout(location = 0) in vec4 position;
layout(location = 1) in vec4 line;
layout(location = 2) in mat3 instance_offset;
layout(location = 3) in vec4 color;
layout(location = 4) in float width;

smooth out vec2 vertex_position;
smooth out vec2 line_start;
smooth out vec2 line_stop;
smooth out vec4 color;
smooth out float width;


void main() {
    vec3 new_position = instance_offset  * vec3(position.xy, 1);
    position = vec4(new_position.xy, position.zw);
    line_start = (matrix * vec3(line.xy, 1.)).xy;
    line_stop = (matrix * vec3(line.zw, 1.)).xy;
    vertex_position = (matrix * vec3(position.xy, 1.)).xy;
    //vertex_position = (matrix * vec3(1., 0., 1.)).xy;
    gl_Position = vec4(vertex_position,  position.z, position.w);
    //gl_Position = vec4(position.xy,  position.z, position.w);
    return;
}

