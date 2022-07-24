<!-- TODO: Refactor this component to use `<component :is="" v-bind="attributesObject"></component>` to avoid all the separate components with `v-if` -->
<!-- TODO: Also rename this component, and probably move the `widget-${direction}` wrapper to be part of `WidgetLayout.vue` as part of its refactor -->

<template>
	<div :class="`widget-${direction}`">
		<template v-for="(component, index) in widgets" :key="index">
			<CheckboxInput v-if="component.props.kind === 'CheckboxInput'" v-bind="component.props" @update:checked="(value: boolean) => updateLayout(component.widgetId, value)" />
			<ColorInput v-if="component.props.kind === 'ColorInput'" v-bind="component.props" v-model:open="open" @update:value="(value: string) => updateLayout(component.widgetId, value)" />
			<DropdownInput
				v-if="component.props.kind === 'DropdownInput'"
				v-bind="component.props"
				v-model:open="open"
				@update:selectedIndex="(value: number) => updateLayout(component.widgetId, value)"
			/>
			<FontInput
				v-if="component.props.kind === 'FontInput'"
				v-bind="component.props"
				v-model:open="open"
				@changeFont="(value: { name: string, style: string, file: string }) => updateLayout(component.widgetId, value)"
			/>
			<IconButton v-if="component.props.kind === 'IconButton'" v-bind="component.props" :action="() => updateLayout(component.widgetId, null)" />
			<IconLabel v-if="component.props.kind === 'IconLabel'" v-bind="component.props" />
			<NumberInput
				v-if="component.props.kind === 'NumberInput'"
				v-bind="component.props"
				@update:value="(value: number) => updateLayout(component.widgetId, value)"
				:incrementCallbackIncrease="() => updateLayout(component.widgetId, 'Increment')"
				:incrementCallbackDecrease="() => updateLayout(component.widgetId, 'Decrement')"
			/>
			<OptionalInput v-if="component.props.kind === 'OptionalInput'" v-bind="component.props" @update:checked="(value: boolean) => updateLayout(component.widgetId, value)" />
			<PopoverButton v-if="component.props.kind === 'PopoverButton'" v-bind="component.props">
				<h3>{{ component.props.header }}</h3>
				<p>{{ component.props.text }}</p>
			</PopoverButton>
			<RadioInput v-if="component.props.kind === 'RadioInput'" v-bind="component.props" @update:selectedIndex="(value: number) => updateLayout(component.widgetId, value)" />
			<Separator v-if="component.props.kind === 'Separator'" v-bind="component.props" />
			<SwatchPairInput v-if="component.props.kind === 'SwatchPairInput'" v-bind="component.props" />
			<TextAreaInput v-if="component.props.kind === 'TextAreaInput'" v-bind="component.props" @commitText="(value: string) => updateLayout(component.widgetId, value)" />
			<TextButton v-if="component.props.kind === 'TextButton'" v-bind="component.props" :action="() => updateLayout(component.widgetId, null)" />
			<TextInput v-if="component.props.kind === 'TextInput'" v-bind="component.props" @commitText="(value: string) => updateLayout(component.widgetId, value)" />
			<TextLabel v-if="component.props.kind === 'TextLabel'" v-bind="withoutValue(component.props)">{{ component.props.value }}</TextLabel>
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
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { WidgetColumn, WidgetRow, isWidgetColumn, isWidgetRow } from "@/wasm-communication/messages";

import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import TextButton from "@/components/widgets/buttons/TextButton.vue";
import CheckboxInput from "@/components/widgets/inputs/CheckboxInput.vue";
import ColorInput from "@/components/widgets/inputs/ColorInput.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import FontInput from "@/components/widgets/inputs/FontInput.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import OptionalInput from "@/components/widgets/inputs/OptionalInput.vue";
import RadioInput from "@/components/widgets/inputs/RadioInput.vue";
import SwatchPairInput from "@/components/widgets/inputs/SwatchPairInput.vue";
import TextAreaInput from "@/components/widgets/inputs/TextAreaInput.vue";
import TextInput from "@/components/widgets/inputs/TextInput.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import Separator from "@/components/widgets/labels/Separator.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

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
		direction() {
			if (isWidgetColumn(this.widgetData)) return "column";
			if (isWidgetRow(this.widgetData)) return "row";
			return "ERROR";
		},
		widgets() {
			if (isWidgetColumn(this.widgetData)) return this.widgetData.columnWidgets;
			if (isWidgetRow(this.widgetData)) return this.widgetData.rowWidgets;
			return [];
		},
	},
	methods: {
		updateLayout(widgetId: bigint, value: unknown) {
			this.editor.instance.update_layout(this.layoutTarget, widgetId, value);
		},
		withoutValue(props: Record<string, unknown>): Record<string, unknown> {
			const { value: _, ...rest } = props;
			return rest;
		},
	},
	components: {
		CheckboxInput,
		ColorInput,
		DropdownInput,
		FontInput,
		IconButton,
		IconLabel,
		NumberInput,
		OptionalInput,
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

