// Allow `import` statements to work with image files in the eyes of the TypeScript compiler.
// This prevents red underlines from showing and lets it know the types of imported variables for image data.
// The actual import is performed by Vite when building, as configured in the `resolve` aliases in `vite.config.ts`.

declare module "*.png" {
	const content: string;
	export default content;
}

declare module "*.jpg" {
	const content: string;
	export default content;
}
