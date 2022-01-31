import { IconName, IconSize } from "@/utilities/icons";

// Text Button
export interface TextButtonWidget {
	kind: "TextButton";
	tooltip?: string;
	message?: string | object;
	callback?: () => void;
	props: TextButtonProps;
}

export interface TextButtonProps {
	// `action` is used via `IconButtonWidget.callback`
	label: string;
	emphasized?: boolean;
	disabled?: boolean;
	minWidth?: number;
	gapAfter?: boolean;
}

// Icon Button
export interface IconButtonWidget {
	kind: "IconButton";
	tooltip?: string;
	message?: string | object;
	callback?: () => void;
	props: IconButtonProps;
}

export interface IconButtonProps {
	// `action` is used via `IconButtonWidget.callback`
	icon: IconName;
	size: IconSize;
	gapAfter?: boolean;
}

// Popover Button
export interface PopoverButtonWidget {
	kind: "PopoverButton";
	tooltip?: string;
	callback?: () => void;
	// popover: WidgetLayout;
	popover: { title: string; text: string }; // TODO: Replace this with a `WidgetLayout` like above for arbitrary layouts
	props: PopoverButtonProps;
}

export interface PopoverButtonProps {
	// `action` is used via `PopoverButtonWidget.callback`
	icon?: PopoverButtonIcon;
}

type Extends<T, U extends T> = U;
export type PopoverButtonIcon = Extends<IconName, "DropdownArrow" | "VerticalEllipsis">;

// Number Input
export interface NumberInputWidget {
	kind: "NumberInput";
	tooltip?: string;
	optionPath: string[];
	props: Omit<NumberInputProps, "value">;
}

export interface NumberInputProps {
	value: number;
	min?: number;
	max?: number;
	incrementBehavior?: IncrementBehavior;
	incrementFactor?: number;
	isInteger?: boolean;
	unit?: string;
	unitIsHiddenWhenEditing?: boolean;
	displayDecimalPlaces?: number;
	label?: string;
	disabled?: boolean;
}

export type IncrementBehavior = "Add" | "Multiply" | "Callback" | "None";
export type IncrementDirection = "Decrease" | "Increase";

// Separator
export type SeparatorDirection = "Horizontal" | "Vertical";
export type SeparatorType = "Related" | "Unrelated" | "Section" | "List";

export interface SeparatorWidget {
	kind: "Separator";
	props: SeparatorProps;
}

export interface SeparatorProps {
	direction?: SeparatorDirection;
	type?: SeparatorType;
}
