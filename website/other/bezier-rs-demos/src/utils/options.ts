import { BEZIER_T_VALUE_VARIANTS, CAP_VARIANTS, JOIN_VARIANTS, SUBPATH_T_VALUE_VARIANTS } from "@/utils/types";

export const tSliderOptions = {
	variable: "t",
	min: 0,
	max: 1,
	step: 0.01,
	default: 0.5,
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

export const separationDiskDiameter = {
	variable: "separation_disk_diameter",
	min: 2.5,
	max: 25,
	step: 0.1,
	default: 5,
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
	options: JOIN_VARIANTS,
};

export const capOptions = {
	variable: "cap",
	default: 0,
	inputType: "dropdown",
	options: CAP_VARIANTS,
};
