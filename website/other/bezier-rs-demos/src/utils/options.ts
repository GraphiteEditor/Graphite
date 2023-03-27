import { BEZIER_T_VALUE_VARIANTS, SUBPATH_T_VALUE_VARIANTS } from "@/utils/types";

export const tSliderOptions = {
	min: 0,
	max: 1,
	step: 0.01,
	default: 0.5,
	variable: "t",
};

export const errorOptions = {
	variable: "error",
	min: 0.1,
	max: 2,
	step: 0.1,
	default: 0.5,
};

export const minimumSeparationOptions = {
	variable: "minimum_separation",
	min: 0.001,
	max: 0.25,
	step: 0.001,
	default: 0.05,
};

export const intersectionErrorOptions = {
	variable: "error",
	min: 0.001,
	max: 0.525,
	step: 0.0025,
	default: 0.02,
};

export const bezierTValueVariantOptions = {
	variable: "TVariant",
	default: 0,
	inputType: "dropdown",
	options: BEZIER_T_VALUE_VARIANTS,
};

export const subpathTValueVariantOptions = {
	variable: "TVariant",
	default: 0,
	inputType: "dropdown",
	options: SUBPATH_T_VALUE_VARIANTS,
};

export const joinOptions = {
	variable: "join",
	default: 0,
	inputType: "dropdown",
	options: ["Bevel", "Miter", "Round"],
};

export const capOptions = {
	variable: "cap",
	default: 0,
	inputType: "dropdown",
	options: ["Butt", "Round", "Square"],
};
