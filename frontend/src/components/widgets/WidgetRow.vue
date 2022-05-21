<template>
	<div :class="`widget-${direction}`">
		<template v-for="(component, index) in widgets" :key="index">
			<!-- TODO: Use `<component :is="" v-bind="attributesObject"></component>` to avoid all the separate components with `v-if` -->
			<CheckboxInput v-if="component.kind === 'CheckboxInput'" v-bind="component.props" @update:checked="(value: boolean) => updateLayout(component.widget_id, value)" />
			<ColorInput v-if="component.kind === 'ColorInput'" v-bind="component.props" v-model:open="open" @update:value="(value: string) => updateLayout(component.widget_id, value)" />
			<DropdownInput v-if="component.kind === 'DropdownInput'" v-bind="component.props" v-model:open="open" @update:selectedIndex="(value: number) => updateLayout(component.widget_id, value)" />
			<FontInput
				v-if="component.kind === 'FontInput'"
				v-bind="component.props"
				v-model:open="open"
				@changeFont="(value: { name: string, style: string, file: string }) => updateLayout(component.widget_id, value)"
			/>
			<IconButton v-if="component.kind === 'IconButton'" v-bind="component.props" :action="() => updateLayout(component.widget_id, null)" />
			<IconLabel v-if="component.kind === 'IconLabel'" v-bind="component.props" />
			<NumberInput
				v-if="component.kind === 'NumberInput'"
				v-bind="component.props"
				@update:value="(value: number) => updateLayout(component.widget_id, value)"
				:incrementCallbackIncrease="() => updateLayout(component.widget_id, 'Increment')"
				:incrementCallbackDecrease="() => updateLayout(component.widget_id, 'Decrement')"
			/>
			<OptionalInput v-if="component.kind === 'OptionalInput'" v-bind="component.props" @update:checked="(value: boolean) => updateLayout(component.widget_id, value)" />
			<PopoverButton v-if="component.kind === 'PopoverButton'">
				<h3>{{ component.props.title }}</h3>
				<p>{{ component.props.text }}</p>
			</PopoverButton>
			<RadioInput v-if="component.kind === 'RadioInput'" v-bind="component.props" @update:selectedIndex="(value: number) => updateLayout(component.widget_id, value)" />
			<Separator v-if="component.kind === 'Separator'" v-bind="component.props" />
			<TextAreaInput v-if="component.kind === 'TextAreaInput'" v-bind="component.props" @commitText="(value: string) => updateLayout(component.widget_id, value)" />
			<TextButton v-if="component.kind === 'TextButton'" v-bind="component.props" :action="() => updateLayout(component.widget_id, null)" />
			<TextInput v-if="component.kind === 'TextInput'" v-bind="component.props" @commitText="(value: string) => updateLayout(component.widget_id, value)" />
			<TextLabel v-if="component.kind === 'TextLabel'" v-bind="withoutValue(component.props)">{{ component.props.value }}</TextLabel>
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

import { WidgetColumn, WidgetRow, isWidgetColumn, isWidgetRow } from "@/interop/messages";

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
import TextAreaInput from "@/components/widgets/inputs/TextAreaInput.vue";
import TextInput from "@/components/widgets/inputs/TextInput.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";
import Separator from "@/components/widgets/separators/Separator.vue";

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
		updateLayout(widgetId: BigInt, value: unknown) {
			this.editor.instance.update_layout(this.layoutTarget, widgetId, value);
		},
		withoutValue(props: Record<string, unknown>): Record<string, unknown> {
			const { value: _, ...rest } = props;
			return rest;
		},
	},
	components: {
		Separator,
		PopoverButton,
		TextButton,
		CheckboxInput,
		NumberInput,
		TextInput,
		IconButton,
		OptionalInput,
		RadioInput,
		DropdownInput,
		TextLabel,
		IconLabel,
		ColorInput,
		FontInput,
		TextAreaInput,
	},
});
</script>

