export function clamp(value: number, min = 0, max = 1) {
	return Math.max(min, Math.min(value, max));
}
