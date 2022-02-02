#version 300 es

precision highp float;

struct Result {
    float dist;
    float side;
};

in vec2 vertex_position;
in vec2 line_start;
in vec2 line_stop;
//layout(location = 0) out vec4 _fs2p_location0;
out vec4 outColor;

Result ud_segment(vec2 p, vec2 a, vec2 b) {
    Result res;
    vec2 ba = (b - a);
    vec2 pa = (p - a);
    float h = clamp((dot(pa, ba) / dot(ba, ba)), 0.0, 1.0);
    res.dist = length((pa - (h * ba)));
    res.side = sign((((b.x - a.x) * (p.y - a.y)) - ((b.y - a.y) * (p.x - a.x))));
    return res;
}

void main() {
    Result res = ud_segment(vertex_position, line_start, line_stop);
    float dist = res.dist;
    float pos = ((dist * 20.0) - 1.0 * trunc((dist * 20.0) / 1.0));
    outColor = vec4(pos, pos, pos, 1.0);
    //out_1.depth = (dist * 6.0);
    gl_FragDepth = dist * 6.;
    return;
}

