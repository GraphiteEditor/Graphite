#version 300 es

precision highp float;

struct Result {
    float dist;
    float side;
};

uniform mat3x2 matrix;
uniform mat3x2 inverse_matrix;
uniform vec2 canvas_resolution;

smooth in vec2 vertex_position;
flat in vec2 line_start;
flat in vec2 line_stop;
flat in vec4 color;
flat in float width;
flat in float zindex;
flat in uint flags;

//layout(location = 0) out vec4 _fs2p_location0;
out vec4 outColor;

#define CLOSED 1u
#define ROUNDED 2u
#define flag(f) ((flags & f) > 0u)

Result ud_segment(vec2 p, vec2 a, vec2 b) {
    Result res;
    vec2 ba = (b - a);
    vec2 pa = (p - a);
    float h = clamp((dot(pa, ba) / dot(ba, ba)), 0.0, 1.0);
    float aspect = canvas_resolution.x / canvas_resolution.y;
    mat2 scale = mat2(canvas_resolution.x / 2., 0., 0., canvas_resolution.y / 2.);
    vec2 dist = pa - (h*ba);
    dist = scale * dist;//inverse(matrix) * vec3(dist, 1.);
    if (flag(ROUNDED)) {
        res.dist = length(dist);
    } else {
        res.dist = max(dist.x, dist.y);
    }
    //res.dist = length(a - b);
    //res.dist = min(length(pa), length(p - b));
    res.side = sign((((b.x - a.x) * (p.y - a.y)) - ((b.y - a.y) * (p.x - a.x))));
    return res;
}

void main() {
    Result res = ud_segment(vertex_position, line_start, line_stop);

    float dist = res.dist;
    float pos = ((dist) - 1.0 * trunc((dist)));
    float new_width = width / 10.;
    float alpha = min(1.,  1. + new_width - dist);
    outColor = vec4(color.rgb, alpha * color.a);// vec4(width , color.yzw);// vec4(color.xyz, pos);
    if (alpha <= 0. && !(flag(CLOSED) && res.side < 0.)) {
        discard;
    }
    gl_FragDepth = (dist * 1.  + zindex) / 100000000.;
    return;
}

