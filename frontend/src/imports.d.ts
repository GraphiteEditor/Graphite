// Allow `import` statements to work with SVG files in the eyes of the TypeScript compiler.
// This prevents red underlines from showing and lets it know the types of imported variables are strings.
// The actual import is performed by the bundler when building.
declare module "*.svg" {
	const content: string;
	export default content;
}
