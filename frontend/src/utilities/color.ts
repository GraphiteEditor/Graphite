import { HSVA, RGBA } from "@/dispatcher/js-messages";

export function hsvaToRgba(hsva: HSVA): RGBA {
	const { h, s, v, a } = hsva;

	const hue = h * 6;
	const hueIntegerPart = Math.floor(hue);
	const hueFractionalPart = hue - hueIntegerPart;
	const hueIntegerMod6 = hueIntegerPart % 6;

	const p = v * (1 - s);
	const q = v * (1 - hueFractionalPart * s);
	const t = v * (1 - (1 - hueFractionalPart) * s);

	const r = Math.round([v, q, p, p, t, v][hueIntegerMod6]);
	const g = Math.round([t, v, v, q, p, p][hueIntegerMod6]);
	const b = Math.round([p, p, t, v, v, q][hueIntegerMod6]);

	return { r, g, b, a };
}

export function rgbaToHsva(rgba: RGBA): HSVA {
	const { r, g, b, a } = rgba;

	const max = Math.max(r, g, b);
	const min = Math.min(r, g, b);

	const d = max - min;
	const s = max === 0 ? 0 : d / max;
	const v = max;

	let h = 0;
	if (max !== min) {
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

	return { h, s, v, a };
}

export function rgbaToDecimalRgba(rgba: RGBA): RGBA {
	const r = rgba.r / 255;
	const g = rgba.g / 255;
	const b = rgba.b / 255;

	return { r, g, b, a: rgba.a };
}
