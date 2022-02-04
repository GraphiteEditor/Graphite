<template>
	<div>{{ widgetRow.name }}</div>
	<div class="widget-row">
		<template v-for="(component, index) in widgetRow.widgets" :key="index">
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
			<IconButton v-if="component.kind === 'IconButton'" v-bind="component.props" :action="() => updateLayout(component.widget_id, null)" />
			<OptionalInput v-if="component.kind === 'OptionalInput'" v-bind="component.props" @update:checked="(value: boolean) => updateLayout(component.widget_id, value)" />
			<RadioInput v-if="component.kind === 'RadioInput'" v-bind="component.props" @update:selectedIndex="(value: number) => updateLayout(component.widget_id, value)" />
			<Separator v-if="component.kind === 'Separator'" v-bind="component.props" />
		</template>
	</div>
</template>

<style lang="scss">
.widget-row {
	height: 100%;
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
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import OptionalInput from "@/components/widgets/inputs/OptionalInput.vue";
import RadioInput from "@/components/widgets/inputs/RadioInput.vue";
import Separator from "@/components/widgets/separators/Separator.vue";

export default defineComponent({
	inject: ["editor"],
	props: {
		widgetRow: { type: Object as PropType<WidgetRow>, required: true },
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
		IconButton,
		OptionalInput,
		RadioInput,
	},
});
</script>

