import type { FillChoiceUI, GradientUI, SRGBA8 } from "/wrapper/pkg/graphite_wasm_wrapper";

// Channels can have any range (0-1, 0-255, 0-100, 0-360) in the context they are being used in, these are just containers for the numbers
export type HSV = { h: number; s: number; v: number };
export type RGB = { r: number; g: number; b: number };

// COLOR FACTORY FUNCTIONS

export function createSRgba8(red: number, green: number, blue: number, alpha: number): SRGBA8 {
	return { red, green, blue, alpha };
}

// Build an `SRGBA8` from HSVA components on the 0..1 range.
export function createSRgba8FromHsva(h: number, s: number, v: number, a: number): SRGBA8 {
	const convert = (n: number): number => {
		const k = (n + h * 6) % 6;
		return v - v * s * Math.max(Math.min(...[k, 4 - k, 1]), 0);
	};

	return {
		red: Math.round(convert(5) * 255),
		green: Math.round(convert(3) * 255),
		blue: Math.round(convert(1) * 255),
		alpha: Math.round(a * 255),
	};
}

// COLOR UTILITY FUNCTIONS

export function isSRgba8(value: unknown): value is SRGBA8 {
	return typeof value === "object" && value !== null && "red" in value;
}

// Parse a CSS color string into an `SRGBA8`. Uses a canvas to delegate parsing to the browser.
export function sRgba8FromCSS(colorCode: string): SRGBA8 | undefined {
	// Allow single-digit hex value inputs
	let colorValue = colorCode.trim();
	if (colorValue.length === 2 && colorValue.charAt(0) === "#" && /[0-9a-f]/i.test(colorValue.charAt(1))) {
		const digit = colorValue.charAt(1);
		colorValue = `#${digit}${digit}${digit}`;
	}

	const canvas = document.createElement("canvas");
	canvas.width = 1;
	canvas.height = 1;
	const context = canvas.getContext("2d", { willReadFrequently: true });
	if (!context) return undefined;

	context.clearRect(0, 0, 1, 1);

	context.fillStyle = "black";
	context.fillStyle = colorValue;
	const comparisonA = context.fillStyle;

	context.fillStyle = "white";
	context.fillStyle = colorValue;
	const comparisonB = context.fillStyle;

	// Invalid color
	if (comparisonA !== comparisonB) {
		// If this color code didn't start with a #, add it and try again
		if (colorValue.trim().charAt(0) !== "#") return sRgba8FromCSS(`#${colorValue.trim()}`);
		return undefined;
	}

	context.fillRect(0, 0, 1, 1);

	const [r, g, b, a] = [...context.getImageData(0, 0, 1, 1).data];
	return createSRgba8(r, g, b, a);
}

export function sRgba8ToHexNoAlpha(color: SRGBA8): string {
	const r = color.red.toString(16).padStart(2, "0");
	const g = color.green.toString(16).padStart(2, "0");
	const b = color.blue.toString(16).padStart(2, "0");

	return `#${r}${g}${b}`;
}

export function sRgba8ToRgb255(color: SRGBA8): RGB {
	return { r: color.red, g: color.green, b: color.blue };
}

export function sRgba8ToRgbCSS(color: SRGBA8): string {
	return `rgb(${color.red}, ${color.green}, ${color.blue})`;
}

export function sRgba8ToRgbaCSS(color: SRGBA8): string {
	return `rgba(${color.red}, ${color.green}, ${color.blue}, ${color.alpha / 255})`;
}

export function sRgba8ToHSV(color: SRGBA8): HSV {
	const r = color.red / 255;
	const g = color.green / 255;
	const b = color.blue / 255;

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

	return { h, s, v };
}

export function sRgba8Opaque(color: SRGBA8): SRGBA8 {
	return createSRgba8(color.red, color.green, color.blue, 255);
}

// WCAG-style relative luminance computed from an `SRGBA8` (alpha composited over white).
export function sRgba8Luminance(color: SRGBA8): number {
	const a = color.alpha / 255;
	// Convert alpha into white
	const r = (color.red / 255) * a + (1 - a);
	const g = (color.green / 255) * a + (1 - a);
	const b = (color.blue / 255) * a + (1 - a);

	// https://stackoverflow.com/a/3943023/775283

	const linearR = r <= 0.04045 ? r / 12.92 : ((r + 0.055) / 1.055) ** 2.4;
	const linearG = g <= 0.04045 ? g / 12.92 : ((g + 0.055) / 1.055) ** 2.4;
	const linearB = b <= 0.04045 ? b / 12.92 : ((b + 0.055) / 1.055) ** 2.4;

	return linearR * 0.2126 + linearG * 0.7152 + linearB * 0.0722;
}

export function sRgba8ContrastingColor(color: SRGBA8 | undefined): "black" | "white" {
	if (!color) return "black";

	const luminance = sRgba8Luminance(color);

	return luminance > Math.sqrt(1.05 * 0.05) - 0.05 ? "black" : "white";
}

export function contrastingOutlineFactor(value: FillChoiceUI, proximityColor: string | [string, string], proximityRange: number): number {
	const pair = Array.isArray(proximityColor) ? [proximityColor[0], proximityColor[1]] : [proximityColor, proximityColor];
	const [range1, range2] = pair.map((color) => sRgba8FromCSS(window.getComputedStyle(document.body).getPropertyValue(color)));

	const contrast = (color: SRGBA8 | undefined): number => {
		if (!color) return 0;

		const lum = sRgba8Luminance(color);
		let rangeLuminance1 = range1 ? sRgba8Luminance(range1) : 0;
		let rangeLuminance2 = range2 ? sRgba8Luminance(range2) : 0;
		[rangeLuminance1, rangeLuminance2] = [Math.min(rangeLuminance1, rangeLuminance2), Math.max(rangeLuminance1, rangeLuminance2)];

		const distance = Math.max(0, rangeLuminance1 - lum, lum - rangeLuminance2);

		return (1 - Math.min(distance / proximityRange, 1)) * (1 - sRgba8ToHSV(color).s);
	};

	const gradient = fillChoiceUIGradient(value);
	if (gradient) {
		if (gradient.color.length === 0) return 0;

		const first = contrast(gradient.color[0]);
		const last = contrast(gradient.color[gradient.color.length - 1]);

		return Math.min(first, last);
	}

	return contrast(fillChoiceUIColor(value));
}

// GRADIENT UTILITY FUNCTIONS

export function isGradientUI(value: unknown): value is GradientUI {
	return typeof value === "object" && value !== null && "position" in value && "midpoint" in value && "color" in value;
}

// FILL CHOICE UTILITY FUNCTIONS

export function fillChoiceUIColor(value: FillChoiceUI): SRGBA8 | undefined {
	if (typeof value === "object" && "Solid" in value) return value.Solid;
	return undefined;
}

export function fillChoiceUIGradient(value: FillChoiceUI): GradientUI | undefined {
	if (typeof value === "object" && "Gradient" in value) return value.Gradient;
	return undefined;
}

export function parseFillChoiceUI(value: unknown): FillChoiceUI {
	if (value === "None" || value === undefined || value === null) return "None";
	if (typeof value === "object" && value !== null && "Solid" in value && isSRgba8(value.Solid)) return { Solid: value.Solid };
	if (typeof value === "object" && value !== null && "Gradient" in value && isGradientUI(value.Gradient)) return { Gradient: value.Gradient };
	return "None";
}
