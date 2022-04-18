<template>
	<div>{{ widgetData.name }}</div>
	<div class="widget-row">
		<template v-for="(component, index) in widgetData.widgets" :key="index">
			<!-- TODO: Use `<component :is="" v-bind="attributesObject"></component>` to avoid all the separate components with `v-if` -->
			<PopoverButton v-if="component.kind === 'PopoverButton'">
				<h3>{{ component.props.title }}</h3>
				<p>{{ component.props.text }}</p>
			</PopoverButton>
			<NumberInput
				v-if="component.kind === 'NumberInput'"
				v-bind="component.props"
				@update:value="(value: number) => updateLayout(component.widget_id, value)"
				:incrementCallbackIncrease="() => updateLayout(component.widget_id, 'Increment')"
				:incrementCallbackDecrease="() => updateLayout(component.widget_id, 'Decrement')"
			/>
			<TextInput v-if="component.kind === 'TextInput'" v-bind="component.props" @commitText="(value: string) => updateLayout(component.widget_id, value)" />
			<TextAreaInput v-if="component.kind === 'TextAreaInput'" v-bind="component.props" @commitText="(value: string) => updateLayout(component.widget_id, value)" />
			<ColorInput v-if="component.kind === 'ColorInput'" v-bind="component.props" @update:value="(value: string) => updateLayout(component.widget_id, value)" />
			<FontInput v-if="component.kind === 'FontInput'" v-bind="component.props" @changeFont="(value: {name:string, style:string, file:string}) => updateLayout(component.widget_id, value)" />
			<IconButton v-if="component.kind === 'IconButton'" v-bind="component.props" :action="() => updateLayout(component.widget_id, null)" />
			<OptionalInput v-if="component.kind === 'OptionalInput'" v-bind="component.props" @update:checked="(value: boolean) => updateLayout(component.widget_id, value)" />
			<RadioInput v-if="component.kind === 'RadioInput'" v-bind="component.props" @update:selectedIndex="(value: number) => updateLayout(component.widget_id, value)" />
			<Separator v-if="component.kind === 'Separator'" v-bind="component.props" />
			<TextLabel v-if="component.kind === 'TextLabel'" v-bind="component.props">{{ component.props.value }}</TextLabel>
			<IconLabel v-if="component.kind === 'IconLabel'" v-bind="component.props" />
		</template>
	</div>
</template>

<style lang="scss">
.widget-row {
	height: 32px;
	flex: 0 0 auto;
	display: flex;
	align-items: center;
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { WidgetRow } from "@/dispatcher/js-messages";

import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import ColorInput from "@/components/widgets/inputs/ColorInput.vue";
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
		widgetData: { type: Object as PropType<WidgetRow>, required: true },
		layoutTarget: { required: true },
	},
	methods: {
		updateLayout(widgetId: BigInt, value: unknown) {
			this.editor.instance.update_layout(this.layoutTarget, widgetId, value);
		},
	},
	components: {
		Separator,
		PopoverButton,
		NumberInput,
		TextInput,
		IconButton,
		OptionalInput,
		RadioInput,
		TextLabel,
		IconLabel,
		ColorInput,
		FontInput,
		TextAreaInput,
	},
});
</script>

