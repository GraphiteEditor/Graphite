/* eslint-disable import/first */

// Demo artwork
import ThumbnailJustAPottedCactus from "@graphite-frontend/assets/images/demo-artwork/thumbnail-just-a-potted-cactus.png";
import ThumbnailProceduralStringLights from "@graphite-frontend/assets/images/demo-artwork/thumbnail-procedural-string-lights.png";
import ThumbnailRedDress from "@graphite-frontend/assets/images/demo-artwork/thumbnail-red-dress.png";
import ThumbnailValleyOfSpires from "@graphite-frontend/assets/images/demo-artwork/thumbnail-valley-of-spires.png";

const DEMO_ARTWORK = {
	ThumbnailJustAPottedCactus,
	ThumbnailProceduralStringLights,
	ThumbnailValleyOfSpires,
	ThumbnailRedDress,
} as const;

// All images
const IMAGE_LIST = {
	...DEMO_ARTWORK,
} as const;

// Exported images and types
export const IMAGES: ImageDefinitionType<typeof IMAGE_LIST> = IMAGE_LIST;
export const IMAGE_BASE64_STRINGS = Object.fromEntries(Object.entries(IMAGES).map(([name, data]) => [name, data]));

// See `icons.ts` for explanation about how this works
type EvaluateType<T> = T extends infer O ? { [K in keyof O]: O[K] } : never;
type ImageDefinitionType<T extends Record<string, string>> = EvaluateType<{ [key in keyof T]: string }>;
