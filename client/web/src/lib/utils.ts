export interface RGB {
	r: number;
	g: number;
	b: number;
	a: number;
}

export interface HSV {
	h: number;
	s: number;
	v: number;
	a: number;
}

export function HSV2RGB(hsv: HSV): RGB {
	let { h } = hsv;
	const { s, v } = hsv;
	h *= 6;
	const i = Math.floor(h);
	const f = h - i;
	const p = v * (1 - s);
	const q = v * (1 - f * s);
	const t = v * (1 - (1 - f) * s);
	const mod = i % 6;
	return {
		r: Math.floor([v, q, p, p, t, v][mod] * 255),
		g: Math.floor([t, v, v, q, p, p][mod] * 255),
		b: Math.floor([p, p, t, v, v, q][mod] * 255),
		a: hsv.a,
	};
}

export function RGB2HSV(rgb: RGB) {
	const { r, g, b } = rgb;
	const max = Math.max(r / 255, g / 255, b / 255);
	const min = Math.min(r / 255, g / 255, b / 255);
	const d = max - min;
	const s = max === 0 ? 0 : d / max;
	const v = max;
	let h = 0;
	if (max === min) {
		h = 0;
	} else {
		switch (max) {
			case r: {
				h = (g - b) / d + (g < b ? 6 : 0);
				break;
			}
			case g: {
				h = (b - r) / d + 2;
				break;
			}
			case b: {
				h = (r - g) / d + 4;
				break;
			}
			default:
		}
		h /= 6;
	}
	return { h, s, v, a: rgb.a };
}

export function clamp(value: number, min = 0, max = 1) {
	return Math.max(min, Math.min(value, max));
}
