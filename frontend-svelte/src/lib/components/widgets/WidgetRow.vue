<!-- TODO: Refactor this component to use `<component :is="" v-bind="attributesObject"></component>` to avoid all the separate components with `v-if` -->
<!-- TODO: Also rename this component, and probably move the `widget-${direction}` wrapper to be part of `WidgetLayout.vue` as part of its refactor -->

<template>
	<div :class="`widget-${direction}`">
		<template v-for="([component, nextIsSuffix], index) in widgetsAndNextSiblingIsSuffix" :key="index">
			<CheckboxInput v-if="component.props.kind === 'CheckboxInput'" v-bind="component.props" @update:checked="(value: boolean) => updateLayout(index, value)" />
			<ColorInput
				v-if="component.props.kind === 'ColorInput'"
				v-bind="component.props"
				v-model:open="open"
				@update:value="(value: unknown) => updateLayout(index, value)"
				:sharpRightCorners="nextIsSuffix"
			/>
			<DropdownInput
				v-if="component.props.kind === 'DropdownInput'"
				v-bind="component.props"
				v-model:open="open"
				@update:selectedIndex="(value: number) => updateLayout(index, value)"
				:sharpRightCorners="nextIsSuffix"
			/>
			<FontInput
				v-if="component.props.kind === 'FontInput'"
				v-bind="component.props"
				v-model:open="open"
				@changeFont="(value: unknown) => updateLayout(index, value)"
				:sharpRightCorners="nextIsSuffix"
			/>
			<ParameterExposeButton v-if="component.props.kind === 'ParameterExposeButton'" v-bind="component.props" :action="() => updateLayout(index, undefined)" />
			<IconButton v-if="component.props.kind === 'IconButton'" v-bind="component.props" :action="() => updateLayout(index, undefined)" :sharpRightCorners="nextIsSuffix" />
			<IconLabel v-if="component.props.kind === 'IconLabel'" v-bind="component.props" />
			<LayerReferenceInput v-if="component.props.kind === 'LayerReferenceInput'" v-bind="component.props" @update:value="(value: BigUint64Array) => updateLayout(index, value)" />
			<NumberInput
				v-if="component.props.kind === 'NumberInput'"
				v-bind="component.props"
				@update:value="debouncer((value: number) => updateLayout(index, value)).updateValue"
				:incrementCallbackIncrease="() => updateLayout(index, 'Increment')"
				:incrementCallbackDecrease="() => updateLayout(index, 'Decrement')"
				:sharpRightCorners="nextIsSuffix"
			/>
			<OptionalInput v-if="component.props.kind === 'OptionalInput'" v-bind="component.props" @update:checked="(value: boolean) => updateLayout(index, value)" />
			<PivotAssist v-if="component.props.kind === 'PivotAssist'" v-bind="component.props" @update:position="(value: string) => updateLayout(index, value)" />
			<PopoverButton v-if="component.props.kind === 'PopoverButton'" v-bind="component.props">
				<TextLabel :bold="true">{{ (component.props as any).header }}</TextLabel>
				<TextLabel :multiline="true">{{ (component.props as any).text }}</TextLabel>
			</PopoverButton>
			<RadioInput v-if="component.props.kind === 'RadioInput'" v-bind="component.props" @update:selectedIndex="(value: number) => updateLayout(index, value)" :sharpRightCorners="nextIsSuffix" />
			<Separator v-if="component.props.kind === 'Separator'" v-bind="component.props" />
			<SwatchPairInput v-if="component.props.kind === 'SwatchPairInput'" v-bind="component.props" />
			<TextAreaInput v-if="component.props.kind === 'TextAreaInput'" v-bind="component.props" @commitText="(value: string) => updateLayout(index, value)" />
			<TextButton v-if="component.props.kind === 'TextButton'" v-bind="component.props" :action="() => updateLayout(index, undefined)" :sharpRightCorners="nextIsSuffix" />
			<BreadcrumbTrailButtons v-if="component.props.kind === 'BreadcrumbTrailButtons'" v-bind="component.props" :action="(index: number) => updateLayout(index, index)" />
			<TextInput v-if="component.props.kind === 'TextInput'" v-bind="component.props" @commitText="(value: string) => updateLayout(index, value)" :sharpRightCorners="nextIsSuffix" />
			<TextLabel v-if="component.props.kind === 'TextLabel'" v-bind="withoutValue(component.props)">{{ (component.props as any).value }}</TextLabel>
		</template>
	</div>
</template>

<style lang="scss" global>
.widget-column {
	flex: 0 0 auto;
	display: flex;
	flex-direction: column;
}

.widget-row {
	flex: 0 0 auto;
	display: flex;
	min-height: 32px;

	> * {
		--widget-height: 24px;
		margin: calc((24px - var(--widget-height)) / 2 + 4px) 0;
		min-height: var(--widget-height);

		&:not(.multiline) {
			line-height: var(--widget-height);
		}

		&.icon-label.size-12 {
			--widget-height: 12px;
		}

		&.icon-label.size-16 {
			--widget-height: 16px;
		}
	}

	// TODO: Target this in a better way than using the tooltip, which will break if changed, or when localized/translated
	.checkbox-input [title="Preserve Aspect Ratio"] {
		margin-bottom: -32px;
		position: relative;

		&::before,
		&::after {
			content: "";
			pointer-events: none;
			position: absolute;
			left: 8px;
			width: 1px;
			height: 16px;
			background: var(--color-7-middlegray);
		}

		&::before {
			top: calc(-4px - 16px);
		}

		&::after {
			bottom: calc(-4px - 16px);
		}
	}
}
</style>

<script lang="ts">


import { debouncer } from "$lib/components/widgets/debounce";
import type { Widget } from "@/wasm-communication/messages";
import { isWidgetColumn, isWidgetRow, type WidgetColumn, type WidgetRow } from "@/wasm-communication/messages";

import PivotAssist from "$lib/components/widgets/assists/PivotAssist.svelte";
import BreadcrumbTrailButtons from "$lib/components/widgets/buttons/BreadcrumbTrailButtons.svelte";
import IconButton from "$lib/components/widgets/buttons/IconButton.svelte";
import ParameterExposeButton from "$lib/components/widgets/buttons/ParameterExposeButton.svelte";
import PopoverButton from "$lib/components/widgets/buttons/PopoverButton.svelte";
import TextButton from "$lib/components/widgets/buttons/TextButton.svelte";
import CheckboxInput from "$lib/components/widgets/inputs/CheckboxInput.svelte";
import ColorInput from "$lib/components/widgets/inputs/ColorInput.svelte";
import DropdownInput from "$lib/components/widgets/inputs/DropdownInput.svelte";
import FontInput from "$lib/components/widgets/inputs/FontInput.svelte";
import LayerReferenceInput from "$lib/components/widgets/inputs/LayerReferenceInput.svelte";
import NumberInput from "$lib/components/widgets/inputs/NumberInput.svelte";
import OptionalInput from "$lib/components/widgets/inputs/OptionalInput.svelte";
import RadioInput from "$lib/components/widgets/inputs/RadioInput.svelte";
import SwatchPairInput from "$lib/components/widgets/inputs/SwatchPairInput.svelte";
import TextAreaInput from "$lib/components/widgets/inputs/TextAreaInput.svelte";
import TextInput from "$lib/components/widgets/inputs/TextInput.svelte";
import IconLabel from "$lib/components/widgets/labels/IconLabel.svelte";
import Separator from "$lib/components/widgets/labels/Separator.svelte";
import TextLabel from "$lib/components/widgets/labels/TextLabel.svelte";

const SUFFIX_WIDGETS = ["PopoverButton"];

export default defineComponent({
	inject: ["editor"],
	props: {
		widgetData: { type: Object as PropType<WidgetColumn | WidgetRow>, required: true },
		layoutTarget: { required: true },
	},
	data() {
		return {
			open: false,
		};
	},
	computed: {
		direction(): "column" | "row" | "ERROR" {
			if (isWidgetColumn(this.widgetData)) return "column";
			if (isWidgetRow(this.widgetData)) return "row";
			return "ERROR";
		},
		widgets() {
			let widgets: Widget[] = [];
			if (isWidgetColumn(this.widgetData)) widgets = this.widgetData.columnWidgets;
			if (isWidgetRow(this.widgetData)) widgets = this.widgetData.rowWidgets;
			return widgets;
		},
		widgetsAndNextSiblingIsSuffix(): [Widget, boolean][] {
			return this.widgets.map((widget, index): [Widget, boolean] => {
				// A suffix widget is one that joins up with this widget at the end with only a 1px gap.
				// It uses the CSS sibling selector to give its own left edge corners zero radius.
				// But this JS is needed to set its preceding sibling widget's right edge corners to zero radius.
				const nextSiblingIsSuffix = SUFFIX_WIDGETS.includes(this.widgets[index + 1]?.props.kind);

				return [widget, nextSiblingIsSuffix];
			});
		},
	},
	methods: {
		updateLayout(index: number, value: unknown) {
			this.editor.instance.updateLayout(this.layoutTarget, this.widgets[index].widgetId, value);
		},
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		withoutValue(props: Record<string, any>): Record<string, unknown> {
			const { value: _, ...rest } = props;
			return rest;
		},
		debouncer,
	},
	components: {
		BreadcrumbTrailButtons,
		CheckboxInput,
		ColorInput,
		DropdownInput,
		FontInput,
		IconButton,
		IconLabel,
		LayerReferenceInput,
		NumberInput,
		OptionalInput,
		ParameterExposeButton,
		PivotAssist,
		PopoverButton,
		RadioInput,
		Separator,
		SwatchPairInput,
		TextAreaInput,
		TextButton,
		TextInput,
		TextLabel,
	},
});
</script>

