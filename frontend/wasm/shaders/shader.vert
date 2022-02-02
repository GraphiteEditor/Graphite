#version 300 es

precision highp float;
precision highp int;

layout(location = 0) in vec4 position;
layout(location = 1) in vec4 line;
smooth out vec2 vertex_position;
smooth out vec2 line_start;
smooth out vec2 line_stop;


void main() {
    line_start = line.xy;
    line_stop = line.zw;
    gl_Position = position;
    vertex_position = gl_Position.xy;
    //gl_Position.yz = vec2(-gl_Position.y, gl_Position.z * 2.0 - gl_Position.w);
    return;
}

