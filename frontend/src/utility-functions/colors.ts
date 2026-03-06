import { sampleInterpolatedGradient } from "@graphite/../wasm/pkg/graphite_wasm";
import type { Color, FillChoice, Gradient } from "@graphite/messages";

// Channels can have any range (0-1, 0-255, 0-100, 0-360) in the context they are being used in, these are just containers for the numbers
export type HSV = { h: number; s: number; v: number };
export type RGB = { r: number; g: number; b: number };
export type OptionalColor = Color & { none: boolean };

// COLOR FACTORY FUNCTIONS

export function createColor(red: number, green: number, blue: number, alpha: number): OptionalColor {
	return { red, green, blue, alpha, none: false };
}

export function createNoneColor(): OptionalColor {
	return { red: 0, green: 0, blue: 0, alpha: 1, none: true };
}

export function createColorFromHSVA(h: number, s: number, v: number, a: number): OptionalColor {
	const convert = (n: number): number => {
		const k = (n + h * 6) % 6;
		return v - v * s * Math.max(Math.min(...[k, 4 - k, 1]), 0);
	};

	return { red: convert(5), green: convert(3), blue: convert(1), alpha: a, none: false };
}

// COLOR UTILITY FUNCTIONS

export function isColor(value: unknown): value is OptionalColor {
	return typeof value === "object" && value !== null && "red" in value;
}

export function colorFromCSS(colorCode: string): OptionalColor | undefined {
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
		if (colorValue.trim().charAt(0) !== "#") return colorFromCSS(`#${colorValue.trim()}`);
		return undefined;
	}

	context.fillRect(0, 0, 1, 1);

	const [r, g, b, a] = [...context.getImageData(0, 0, 1, 1).data];
	return createColor(r / 255, g / 255, b / 255, a / 255);
}

export function colorEquals(c1: OptionalColor, c2: OptionalColor): boolean {
	if (c1.none !== c2.none) return false;
	if (c1.none && c2.none) return true;
	return Math.abs(c1.red - c2.red) < 1e-6 && Math.abs(c1.green - c2.green) < 1e-6 && Math.abs(c1.blue - c2.blue) < 1e-6 && Math.abs(c1.alpha - c2.alpha) < 1e-6;
}

export function colorToHexNoAlpha(color: OptionalColor): string | undefined {
	if (color.none) return undefined;

	const r = Math.round(color.red * 255)
		.toString(16)
		.padStart(2, "0");
	const g = Math.round(color.green * 255)
		.toString(16)
		.padStart(2, "0");
	const b = Math.round(color.blue * 255)
		.toString(16)
		.padStart(2, "0");

	return `#${r}${g}${b}`;
}

export function colorToHexOptionalAlpha(color: OptionalColor): string | undefined {
	if (color.none) return undefined;

	const hex = colorToHexNoAlpha(color);
	const a = Math.round(color.alpha * 255)
		.toString(16)
		.padStart(2, "0");

	return a === "ff" ? hex : `${hex}${a}`;
}

export function colorToRgb255(color: OptionalColor): RGB | undefined {
	if (color.none) return undefined;

	return {
		r: Math.round(color.red * 255),
		g: Math.round(color.green * 255),
		b: Math.round(color.blue * 255),
	};
}

export function colorToRgbCSS(color: OptionalColor): string | undefined {
	const rgb = colorToRgb255(color);
	if (!rgb) return undefined;

	return `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`;
}

export function colorToRgbaCSS(color: OptionalColor): string | undefined {
	const rgb = colorToRgb255(color);
	if (!rgb) return undefined;

	return `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${color.alpha})`;
}

export function colorToHSV(color: OptionalColor): HSV | undefined {
	if (color.none) return undefined;

	const { red: r, green: g, blue: b } = color;

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

export function colorOpaque(color: OptionalColor): OptionalColor | undefined {
	if (color.none) return undefined;

	return createColor(color.red, color.green, color.blue, 1);
}

export function colorLuminance(color: OptionalColor): number | undefined {
	if (color.none) return undefined;

	// Convert alpha into white
	const r = color.red * color.alpha + (1 - color.alpha);
	const g = color.green * color.alpha + (1 - color.alpha);
	const b = color.blue * color.alpha + (1 - color.alpha);

	// https://stackoverflow.com/a/3943023/775283

	const linearR = r <= 0.04045 ? r / 12.92 : ((r + 0.055) / 1.055) ** 2.4;
	const linearG = g <= 0.04045 ? g / 12.92 : ((g + 0.055) / 1.055) ** 2.4;
	const linearB = b <= 0.04045 ? b / 12.92 : ((b + 0.055) / 1.055) ** 2.4;

	return linearR * 0.2126 + linearG * 0.7152 + linearB * 0.0722;
}

export function colorContrastingColor(color: OptionalColor): "black" | "white" {
	if (color.none) return "black";

	const luminance = colorLuminance(color);

	return luminance && luminance > Math.sqrt(1.05 * 0.05) - 0.05 ? "black" : "white";
}

export function contrastingOutlineFactor(value: FillChoice, proximityColor: string | [string, string], proximityRange: number): number {
	const pair = Array.isArray(proximityColor) ? [proximityColor[0], proximityColor[1]] : [proximityColor, proximityColor];
	const [range1, range2] = pair.map((color) => colorFromCSS(window.getComputedStyle(document.body).getPropertyValue(color)) || createNoneColor());

	const contrast = (color: OptionalColor): number => {
		const lum = colorLuminance(color) || 0;
		let rangeLuminance1 = colorLuminance(range1) || 0;
		let rangeLuminance2 = colorLuminance(range2) || 0;
		[rangeLuminance1, rangeLuminance2] = [Math.min(rangeLuminance1, rangeLuminance2), Math.max(rangeLuminance1, rangeLuminance2)];

		const distance = Math.max(0, rangeLuminance1 - lum, lum - rangeLuminance2);

		return (1 - Math.min(distance / proximityRange, 1)) * (1 - (colorToHSV(color)?.s || 0));
	};

	if (isGradient(value)) {
		if (value.color.length === 0) return 0;

		const first = contrast(value.color[0]);
		const last = contrast(value.color[value.color.length - 1]);

		return Math.min(first, last);
	}

	return contrast(value);
}

// GRADIENT UTILITY FUNCTIONS

export function isGradient(value: unknown): value is Gradient {
	return typeof value === "object" && value !== null && "position" in value && "midpoint" in value;
}

export function gradientToLinearGradientCSS(gradient: Gradient): string {
	if (gradient.position.length === 1) {
		return `linear-gradient(to right, ${colorToHexOptionalAlpha(gradient.color[0])} 0%, ${colorToHexOptionalAlpha(gradient.color[0])} 100%)`;
	}

	const pieces = sampleInterpolatedGradient(new Float64Array(gradient.position), new Float64Array(gradient.midpoint), gradient.color, false);
	return `linear-gradient(to right, ${pieces})`;
}

export function gradientFirstColor(gradient: Gradient): OptionalColor | undefined {
	return gradient.color[0];
}

export function gradientLastColor(gradient: Gradient): OptionalColor | undefined {
	return gradient.color[gradient.color.length - 1];
}

// FILL CHOICE UTILITY FUNCTIONS

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function parseFillChoice(value: any): FillChoice {
	if (isColor(value)) return value;
	if (isGradient(value)) return value;

	const gradient: Gradient | undefined = value["Gradient"];
	if (gradient) {
		const color = gradient.color.map((c) => createColor(c.red, c.green, c.blue, c.alpha));
		return { ...gradient, color };
	}

	const solid = value["Solid"];
	if (solid) return createColor(solid.red, solid.green, solid.blue, solid.alpha);

	return createNoneColor();
}
