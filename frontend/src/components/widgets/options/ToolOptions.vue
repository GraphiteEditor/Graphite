<template>
	<div class="tool-options">
		<template v-for="(option, index) in toolOptions[activeTool] || []" :key="index">
			<!-- TODO: Use `<component :is="" v-bind="attributesObject"></component>` to avoid all the separate components with `v-if` -->
			<IconButton v-if="option.kind === 'IconButton'" :action="() => handleIconButtonAction(option)" :title="option.tooltip" v-bind="option.props" />
			<PopoverButton v-if="option.kind === 'PopoverButton'" :title="option.tooltip" :action="option.callback" v-bind="option.props">
				<h3>{{ option.popover.title }}</h3>
				<p>{{ option.popover.text }}</p>
			</PopoverButton>
			<NumberInput v-if="option.kind === 'NumberInput'" v-model:value="option.props.value" @update:value="option.callback" :title="option.tooltip" v-bind="option.props" />
			<Separator v-if="option.kind === 'Separator'" v-bind="option.props" />
		</template>
	</div>
</template>

<style lang="scss">
.tool-options {
	height: 100%;
	flex: 0 0 auto;
	display: flex;
	align-items: center;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { comingSoon } from "@/utilities/errors";

import { WidgetRow, SeparatorType, IconButtonWidget } from "@/components/widgets/widgets";
import Separator from "@/components/widgets/separators/Separator.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";

const wasm = import("@/../wasm/pkg");

export default defineComponent({
	props: {
		activeTool: { type: String },
	},
	computed: {},
	methods: {
		async setToolOptions(newValue: number) {
			// TODO: Each value-input widget (i.e. not a button) should map to a field in an options struct,
			// and updating a widget should send the whole updated struct to the backend.
			// Later, it could send a single-field update to the backend.

			const { set_tool_options } = await wasm;
			// This is a placeholder call, using the Shape tool as an example
			set_tool_options(this.$props.activeTool || "", { Shape: { shape_type: { Polygon: { vertices: newValue } } } });
		},
		async sendToolMessage(message: string | object) {
			const { send_tool_message } = await wasm;
			send_tool_message(this.$props.activeTool || "", message);
		},
		handleIconButtonAction(option: IconButtonWidget) {
			if (option.message) {
				this.sendToolMessage(option.message);
				return;
			}

			if (option.callback) {
				option.callback();
				return;
			}

			comingSoon();
		},
	},
	data() {
		const toolOptions: Record<string, WidgetRow> = {
			Select: [
				{ kind: "IconButton", message: { Align: ["X", "Min"] }, tooltip: "Align Left", props: { icon: "AlignLeft", size: 24 } },
				{ kind: "IconButton", message: { Align: ["X", "Center"] }, tooltip: "Align Horizontal Center", props: { icon: "AlignHorizontalCenter", size: 24 } },
				{ kind: "IconButton", message: { Align: ["X", "Max"] }, tooltip: "Align Right", props: { icon: "AlignRight", size: 24 } },

				{ kind: "Separator", props: { type: SeparatorType.Unrelated } },

				{ kind: "IconButton", message: { Align: ["Y", "Min"] }, tooltip: "Align Top", props: { icon: "AlignTop", size: 24 } },
				{ kind: "IconButton", message: { Align: ["Y", "Center"] }, tooltip: "Align Vertical Center", props: { icon: "AlignVerticalCenter", size: 24 } },
				{ kind: "IconButton", message: { Align: ["Y", "Max"] }, tooltip: "Align Bottom", props: { icon: "AlignBottom", size: 24 } },

				{ kind: "Separator", props: { type: SeparatorType.Related } },

				{
					kind: "PopoverButton",
					popover: {
						title: "Align",
						text: "The contents of this popover menu are coming soon",
					},
					props: {},
				},

				{ kind: "Separator", props: { type: SeparatorType.Section } },

				{ kind: "IconButton", message: "FlipHorizontal", tooltip: "Flip Horizontal", props: { icon: "FlipHorizontal", size: 24 } },
				{ kind: "IconButton", message: "FlipVertical", tooltip: "Flip Vertical", props: { icon: "FlipVertical", size: 24 } },

				{ kind: "Separator", props: { type: SeparatorType.Related } },

				{
					kind: "PopoverButton",
					popover: {
						title: "Flip",
						text: "The contents of this popover menu are coming soon",
					},
					props: {},
				},

				{ kind: "Separator", props: { type: SeparatorType.Section } },

				{ kind: "IconButton", tooltip: "Boolean Union", callback: () => comingSoon(197), props: { icon: "BooleanUnion", size: 24 } },
				{ kind: "IconButton", tooltip: "Boolean Subtract Front", callback: () => comingSoon(197), props: { icon: "BooleanSubtractFront", size: 24 } },
				{ kind: "IconButton", tooltip: "Boolean Subtract Back", callback: () => comingSoon(197), props: { icon: "BooleanSubtractBack", size: 24 } },
				{ kind: "IconButton", tooltip: "Boolean Intersect", callback: () => comingSoon(197), props: { icon: "BooleanIntersect", size: 24 } },
				{ kind: "IconButton", tooltip: "Boolean Difference", callback: () => comingSoon(197), props: { icon: "BooleanDifference", size: 24 } },

				{ kind: "Separator", props: { type: SeparatorType.Related } },

				{
					kind: "PopoverButton",
					popover: {
						title: "Boolean",
						text: "The contents of this popover menu are coming soon",
					},
					props: {},
				},
			],
			Shape: [{ kind: "NumberInput", callback: this.setToolOptions, props: { value: 6, min: 3, isInteger: true, label: "Sides" } }],
		};

		return {
			toolOptions,
			SeparatorType,
			comingSoon,
		};
	},
	components: {
		Separator,
		IconButton,
		PopoverButton,
		NumberInput,
	},
});
</script>
