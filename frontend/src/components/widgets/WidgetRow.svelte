<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { debouncer } from "@/components/widgets/debounce";
import { isWidgetColumn, isWidgetRow, type WidgetColumn, type WidgetRow, type Widget } from "@/wasm-communication/messages";

import PivotAssist from "@/components/widgets/assists/PivotAssist.vue";
import BreadcrumbTrailButtons from "@/components/widgets/buttons/BreadcrumbTrailButtons.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import ParameterExposeButton from "@/components/widgets/buttons/ParameterExposeButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import TextButton from "@/components/widgets/buttons/TextButton.vue";
import CheckboxInput from "@/components/widgets/inputs/CheckboxInput.vue";
import ColorInput from "@/components/widgets/inputs/ColorInput.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import FontInput from "@/components/widgets/inputs/FontInput.vue";
import LayerReferenceInput from "@/components/widgets/inputs/LayerReferenceInput.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import OptionalInput from "@/components/widgets/inputs/OptionalInput.vue";
import RadioInput from "@/components/widgets/inputs/RadioInput.vue";
import SwatchPairInput from "@/components/widgets/inputs/SwatchPairInput.vue";
import TextAreaInput from "@/components/widgets/inputs/TextAreaInput.vue";
import TextInput from "@/components/widgets/inputs/TextInput.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import Separator from "@/components/widgets/labels/Separator.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

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

<style lang="scss">
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
