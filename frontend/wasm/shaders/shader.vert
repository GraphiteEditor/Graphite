#version 300 es

precision highp float;
precision highp int;

uniform mat3 matrix;

layout(location = 0) in vec4 position;
layout(location = 1) in vec4 line;
smooth out vec2 vertex_position;
smooth out vec2 line_start;
smooth out vec2 line_stop;


void main() {
    line_start = (matrix * vec3(line.xy, 1.)).xy;
    line_stop = (matrix * vec3(line.zw, 1.)).xy;
    vertex_position = (matrix * vec3(position.xy, 1.)).xy;
    //vertex_position = (matrix * vec3(1., 0., 1.)).xy;
    gl_Position = vec4(vertex_position,  position.z, position.w);
    //gl_Position = vec4(position.xy,  position.z, position.w);
    return;
}

