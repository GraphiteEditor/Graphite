<template>
	<div class="tool-options">
		<template v-for="(option, index) in toolOptions[activeTool] || []" :key="index">
			<!-- TODO: Use `<component :is="" v-bind="attributesObject"></component>` to avoid all the separate components with `v-if` -->
			<IconButton v-if="option.kind === 'IconButton'" :action="() => handleIconButtonAction(option)" :title="option.tooltip" v-bind="option.props" />
			<PopoverButton v-if="option.kind === 'PopoverButton'" :title="option.tooltip" :action="option.callback" v-bind="option.props">
				<h3>{{ option.popover.title }}</h3>
				<p>{{ option.popover.text }}</p>
			</PopoverButton>
			<NumberInput
				v-if="option.kind === 'NumberInput'"
				@update:value="(value) => updateToolOptions(option.optionPath, value)"
				:title="option.tooltip"
				:value="getToolOption(option.optionPath)"
				v-bind="option.props"
			/>
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
		currentToolOptions: { type: Object },
	},
	computed: {
		activeToolOptions(): Record<string, object> {
			const toolOptions = this.currentToolOptions || {};
			return toolOptions[this.activeTool || ""];
		},
	},
	methods: {
		async updateToolOptions(path: string[], newValue: number) {
			this.setToolOption(path, newValue);
			(await wasm).set_tool_options(this.activeTool || "", this.activeToolOptions);
		},
		async sendToolMessage(message: string | object) {
			(await wasm).send_tool_message(this.activeTool || "", message);
		},
		// Traverses the given path and returns the direct parent of the option
		getRecordContainingOption(optionPath: string[]): Record<string, number> {
			const allButLast = optionPath.slice(0, -1);
			let value = this.activeToolOptions as Record<string, object | number>;
			[this.activeTool || "", ...allButLast].forEach((attr) => {
				value = value[attr] as Record<string, object | number>;
			});
			return value as Record<string, number>;
		},
		// Traverses the given path into the active tool's option struct, and sets the value at the path tail
		setToolOption(optionPath: string[], newValue: number) {
			const last = optionPath.slice(-1)[0];
			const recordContainingOption = this.getRecordContainingOption(optionPath);
			recordContainingOption[last] = newValue;
		},
		// Traverses the given path into the active tool's option struct, and returns the value at the path tail
		getToolOption(optionPath: string[]): number {
			const last = optionPath.slice(-1)[0];
			const recordContainingOption = this.getRecordContainingOption(optionPath);
			return recordContainingOption[last];
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
			Shape: [{ kind: "NumberInput", optionPath: ["shape_type", "Polygon", "vertices"], props: { min: 3, isInteger: true, label: "Sides" } }],
			Line: [{ kind: "NumberInput", optionPath: ["weight"], props: { min: 1, isInteger: true, unit: " px", label: "Weight" } }],
			Pen: [{ kind: "NumberInput", optionPath: ["weight"], props: { min: 1, isInteger: true, unit: " px", label: "Weight" } }],
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
