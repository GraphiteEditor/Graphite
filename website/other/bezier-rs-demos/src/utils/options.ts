import { BEZIER_T_VALUE_VARIANTS, CAP_VARIANTS, JOIN_VARIANTS, SUBPATH_T_VALUE_VARIANTS } from "@/utils/types";

export const tSliderOptions = {
	variable: "t",
	inputType: "slider",
	min: 0,
	max: 1,
	step: 0.01,
	default: 0.5,
};

export const errorOptions = {
	variable: "error",
	inputType: "slider",
	min: 0.1,
	max: 2,
	step: 0.1,
	default: 0.5,
};

export const minimumSeparationOptions = {
	variable: "minimum_separation",
	inputType: "slider",
	min: 0.001,
	max: 0.25,
	step: 0.001,
	default: 0.05,
};

export const intersectionErrorOptions = {
	variable: "error",
	inputType: "slider",
	min: 0.001,
	max: 0.525,
	step: 0.0025,
	default: 0.02,
};

export const separationDiskDiameter = {
	variable: "separation_disk_diameter",
	inputType: "slider",
	min: 2.5,
	max: 25,
	step: 0.1,
	default: 5,
};

export const bezierTValueVariantOptions = {
	variable: "TVariant",
	inputType: "dropdown",
	default: 0,
	options: BEZIER_T_VALUE_VARIANTS,
};

export const subpathTValueVariantOptions = {
	variable: "TVariant",
	inputType: "dropdown",
	default: 0,
	options: SUBPATH_T_VALUE_VARIANTS,
};

export const joinOptions = {
	variable: "join",
	inputType: "dropdown",
	default: 0,
	options: JOIN_VARIANTS,
};

export const capOptions = {
	variable: "cap",
	inputType: "dropdown",
	default: 0,
	options: CAP_VARIANTS,
};
