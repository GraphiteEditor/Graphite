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
	const r = Math.round([v, q, p, p, t, v][mod]);
	const g = Math.round([t, v, v, q, p, p][mod]);
	const b = Math.round([p, p, t, v, v, q][mod]);
	return { r, g, b, a: hsv.a };
}

export function RGB2HSV(rgb: RGB) {
	const { r, g, b } = rgb;
	const max = Math.max(r, g, b);
	const min = Math.min(r, g, b);
	const d = max - min;
	const s = max === 0 ? 0 : d / max;
	const v = max;
	let h = 0;
	if (max === min) {
		h = 0;
	} else {
		switch (max) {
			case r:
				h = (g - b) / d + (g < b ? 6 : 0);
				break;
			case g:
				h = (b - r) / d + 2;
				break;
			case b:
				h = (r - g) / d + 4;
				break;
			default:
		}
		h /= 6;
	}
	return { h, s, v, a: rgb.a };
}

export function RGB2Floats(rgb: RGB) {
	const r = rgb.r / 255;
	const g = rgb.g / 255;
	const b = rgb.b / 255;
	return { r, g, b, a: rgb.a };
}

export function clamp(value: number, min = 0, max = 1) {
	return Math.max(min, Math.min(value, max));
}
