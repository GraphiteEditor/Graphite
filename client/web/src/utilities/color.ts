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

export function hsvToRgb(hsv: HSV): RGB {
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

export function rgbToHsv(rgb: RGB) {
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

export function rgbToDecimalRgb(rgb: RGB) {
	const r = rgb.r / 255;
	const g = rgb.g / 255;
	const b = rgb.b / 255;
	return { r, g, b, a: rgb.a };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function isRGB(data: any): data is RGB {
	if (typeof data !== "object" || data === null) return false;
	return (
		typeof data.r === "number" &&
		!Number.isNaN(data.r) &&
		typeof data.g === "number" &&
		!Number.isNaN(data.g) &&
		typeof data.b === "number" &&
		!Number.isNaN(data.b) &&
		typeof data.a === "number" &&
		!Number.isNaN(data.a)
	);
}
