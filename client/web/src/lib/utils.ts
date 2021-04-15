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

export type RGBFloats = RGB;

export function RGB2Floats(rgb: RGB, floats: RGBFloats) {
	floats.r = rgb.r / 0xff;
	floats.g = rgb.g / 0xff;
	floats.b = rgb.b / 0xff;
	return floats;
}

export function Floats2RGB(floats: RGBFloats, rgb: RGB) {
	rgb.r = Math.floor(floats.r * 255);
	rgb.g = Math.floor(floats.g * 255);
	rgb.b = Math.floor(floats.b * 255);
	return rgb;
}

export function Hex2RGB(hex: string | number, rgb: RGB) {
	let h: number;
	if (typeof hex === "string") {
		h = parseInt(hex, 16);
	} else {
		h = hex;
	}
	rgb.r = (h >> 16) & 0xff;
	rgb.g = (h >> 8) & 0xff;
	rgb.b = h & 0xff;
	return rgb;
}

export function RGB2Hex(rgb: RGB) {
	let hex = "";
	hex += rgb.r.toString(16).padStart(2, "0");
	hex += rgb.g.toString(16).padStart(2, "0");
	hex += rgb.b.toString(16).padStart(2, "0");
	return parseInt(hex, 16);
}

export function Hex2String(h: number) {
	let hex = "";
	hex += ((h >> 16) & 0xff).toString(16).padStart(2, "0");
	hex += ((h >> 8) & 0xff).toString(16).padStart(2, "0");
	hex += (h & 0xff).toString(16).padStart(2, "0");
	return hex;
}

export function HSV2Floats(hsv: HSV, rgb: RGBFloats) {
	let { h } = hsv;
	const { s, v } = hsv;
	h *= 6;
	const i = Math.floor(h);
	const f = h - i;
	const p = v * (1 - s);
	const q = v * (1 - f * s);
	const t = v * (1 - (1 - f) * s);
	const mod = i % 6;
	rgb.r = [v, q, p, p, t, v][mod];
	rgb.g = [t, v, v, q, p, p][mod];
	rgb.b = [p, p, t, v, v, q][mod];
	return rgb;
}

export function HSV2RGB(hsv: HSV, rgb: RGB) {
	HSV2Floats(hsv, rgb);
	return RGB2Floats(rgb, rgb);
}

export function Floats2HSV(rgb: RGBFloats, hsv: HSV) {
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
		}
		h /= 6;
	}
	hsv.h = h;
	hsv.s = s;
	hsv.v = v;
	return hsv;
}

export function RGB2HSV(rgb: RGB, hsv: HSV) {
	return Floats2HSV(
		{
			r: rgb.r / 255,
			g: rgb.g / 255,
			b: rgb.b / 255,
			a: rgb.a,
		},
		hsv
	);
}

export function clamp(value: number, min: number, max: number) {
	return Math.max(min, Math.min(value, max));
}
