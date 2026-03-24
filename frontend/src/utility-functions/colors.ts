import { sampleInterpolatedGradient } from "/wrapper/pkg/graphite_wasm_wrapper";
import type { Color, FillChoice, GradientStops } from "/wrapper/pkg/graphite_wasm_wrapper";

// Channels can have any range (0-1, 0-255, 0-100, 0-360) in the context they are being used in, these are just containers for the numbers
export type HSV = { h: number; s: number; v: number };
export type RGB = { r: number; g: number; b: number };

// COLOR FACTORY FUNCTIONS

export function createColor(red: number, green: number, blue: number, alpha: number): Color {
	return { red, green, blue, alpha };
}

export function createColorFromHSVA(h: number, s: number, v: number, a: number): Color {
	const convert = (n: number): number => {
		const k = (n + h * 6) % 6;
		return v - v * s * Math.max(Math.min(...[k, 4 - k, 1]), 0);
	};

	return { red: convert(5), green: convert(3), blue: convert(1), alpha: a };
}

// COLOR UTILITY FUNCTIONS

export function isColor(value: unknown): value is Color {
	return typeof value === "object" && value !== null && "red" in value;
}

export function colorFromCSS(colorCode: string): Color | undefined {
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

export function colorEquals(c1: Color | undefined, c2: Color | undefined): boolean {
	if (c1 === undefined && c2 === undefined) return true;
	if (c1 === undefined || c2 === undefined) return false;
	return Math.abs(c1.red - c2.red) < 1e-6 && Math.abs(c1.green - c2.green) < 1e-6 && Math.abs(c1.blue - c2.blue) < 1e-6 && Math.abs(c1.alpha - c2.alpha) < 1e-6;
}

export function colorToHexNoAlpha(color: Color): string {
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

export function colorToHexOptionalAlpha(color: Color): string {
	const hex = colorToHexNoAlpha(color);
	const a = Math.round(color.alpha * 255)
		.toString(16)
		.padStart(2, "0");

	return a === "ff" ? hex : `${hex}${a}`;
}

export function colorToRgb255(color: Color): RGB {
	return {
		r: Math.round(color.red * 255),
		g: Math.round(color.green * 255),
		b: Math.round(color.blue * 255),
	};
}

export function colorToRgbCSS(color: Color): string {
	const rgb = colorToRgb255(color);

	return `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`;
}

export function colorToRgbaCSS(color: Color): string {
	const rgb = colorToRgb255(color);

	return `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${color.alpha})`;
}

export function colorToHSV(color: Color): HSV {
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

export function colorOpaque(color: Color): Color {
	return createColor(color.red, color.green, color.blue, 1);
}

export function colorLuminance(color: Color): number {
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

export function colorContrastingColor(color: Color | undefined): "black" | "white" {
	if (!color) return "black";

	const luminance = colorLuminance(color);

	return luminance > Math.sqrt(1.05 * 0.05) - 0.05 ? "black" : "white";
}

export function contrastingOutlineFactor(value: FillChoice, proximityColor: string | [string, string], proximityRange: number): number {
	const pair = Array.isArray(proximityColor) ? [proximityColor[0], proximityColor[1]] : [proximityColor, proximityColor];
	const [range1, range2] = pair.map((color) => colorFromCSS(window.getComputedStyle(document.body).getPropertyValue(color)));

	const contrast = (color: Color | undefined): number => {
		if (!color) return 0;

		const lum = colorLuminance(color);
		let rangeLuminance1 = range1 ? colorLuminance(range1) : 0;
		let rangeLuminance2 = range2 ? colorLuminance(range2) : 0;
		[rangeLuminance1, rangeLuminance2] = [Math.min(rangeLuminance1, rangeLuminance2), Math.max(rangeLuminance1, rangeLuminance2)];

		const distance = Math.max(0, rangeLuminance1 - lum, lum - rangeLuminance2);

		return (1 - Math.min(distance / proximityRange, 1)) * (1 - colorToHSV(color).s);
	};

	const gradientStops = fillChoiceGradientStops(value);
	if (gradientStops) {
		if (gradientStops.color.length === 0) return 0;

		const first = contrast(gradientStops.color[0]);
		const last = contrast(gradientStops.color[gradientStops.color.length - 1]);

		return Math.min(first, last);
	}

	return contrast(fillChoiceColor(value));
}

// GRADIENT UTILITY FUNCTIONS

export function isGradientStops(value: unknown): value is GradientStops {
	return typeof value === "object" && value !== null && "position" in value && "midpoint" in value && "color" in value;
}

export function gradientToLinearGradientCSS(gradient: GradientStops): string {
	if (gradient.position.length === 1) {
		return `linear-gradient(to right, ${colorToHexOptionalAlpha(gradient.color[0])} 0%, ${colorToHexOptionalAlpha(gradient.color[0])} 100%)`;
	}

	const pieces = sampleInterpolatedGradient(new Float64Array(gradient.position), new Float64Array(gradient.midpoint), gradient.color, false);
	return `linear-gradient(to right, ${pieces})`;
}

export function gradientFirstColor(gradient: GradientStops): Color | undefined {
	return gradient.color[0];
}

export function gradientLastColor(gradient: GradientStops): Color | undefined {
	return gradient.color[gradient.color.length - 1];
}

// FILL CHOICE UTILITY FUNCTIONS

export function fillChoiceColor(value: FillChoice): Color | undefined {
	if (typeof value === "object" && "Solid" in value) return value.Solid;
	return undefined;
}

export function fillChoiceGradientStops(value: FillChoice): GradientStops | undefined {
	if (typeof value === "object" && "Gradient" in value) return value.Gradient;
	return undefined;
}

export function parseFillChoice(value: unknown): FillChoice {
	if (value === "None" || value === undefined || value === null) return "None";
	if (typeof value === "object" && value !== null && "Solid" in value && isColor(value.Solid)) return { Solid: value.Solid };
	if (typeof value === "object" && value !== null && "Gradient" in value && isGradientStops(value.Gradient)) return { Gradient: value.Gradient };
	return "None";
}
