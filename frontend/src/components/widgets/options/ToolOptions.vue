<template>
	<LayoutRow class="tool-options">
		<template v-for="(option, index) in toolOptionsWidgets[activeTool] || []" :key="index">
			<!-- TODO: Use `<component :is="" v-bind="attributesObject"></component>` to avoid all the separate components with `v-if` -->
			<IconButton v-if="option.kind === 'IconButton'" :action="() => handleIconButtonAction(option)" :title="option.tooltip" v-bind="option.props" />
			<PopoverButton v-if="option.kind === 'PopoverButton'" :title="option.tooltip" :action="option.callback" v-bind="option.props">
				<h3>{{ option.popover.title }}</h3>
				<p>{{ option.popover.text }}</p>
			</PopoverButton>
			<NumberInput
				v-if="option.kind === 'NumberInput'"
				@update:value="(value: number) => updateToolOptions(option.optionPath, value)"
				:title="option.tooltip"
				:value="getToolOption(option.optionPath)"
				v-bind="option.props"
			/>
			<Separator v-if="option.kind === 'Separator'" v-bind="option.props" />
		</template>
	</LayoutRow>
</template>

<style lang="scss">
.tool-options {
	height: 100%;
	flex: 0 0 auto;
	align-items: center;
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { ToolName } from "@/dispatcher/js-messages";
import { WidgetRow, IconButtonWidget } from "@/utilities/widgets";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import Separator from "@/components/widgets/separators/Separator.vue";

export default defineComponent({
	inject: ["editor", "dialog"],
	props: {
		activeTool: { type: String as PropType<ToolName>, required: true },
		activeToolOptions: { type: Object as PropType<Record<string, object>>, required: true },
	},
	methods: {
		async updateToolOptions(path: string[], newValue: number) {
			this.setToolOption(path, newValue);
			this.editor.instance.set_tool_options(this.activeTool || "", this.activeToolOptions);
		},
		async sendToolMessage(message: string | object) {
			this.editor.instance.send_tool_message(this.activeTool || "", message);
		},
		// Traverses the given path and returns the direct parent of the option
		getRecordContainingOption(optionPath: string[]): Record<string, number> {
			// TODO: Formalize types and avoid casting with `as`
			let currentRecord = this.activeToolOptions as Record<string, object | number>;

			const allButLastOptions = optionPath.slice(0, -1);
			[this.activeTool || "", ...allButLastOptions].forEach((attr) => {
				// Dig into the tree in each loop iteration
				currentRecord = currentRecord[attr] as Record<string, object | number>;
			});

			return currentRecord as Record<string, number>;
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

			this.dialog.comingSoon();
		},
	},
	data() {
		const toolOptionsWidgets: Record<ToolName, WidgetRow> = {
			Select: [
				{ kind: "IconButton", message: { Align: { axis: "X", aggregate: "Min" } }, tooltip: "Align Left", props: { icon: "AlignLeft", size: 24 } },
				{ kind: "IconButton", message: { Align: { axis: "X", aggregate: "Center" } }, tooltip: "Align Horizontal Center", props: { icon: "AlignHorizontalCenter", size: 24 } },
				{ kind: "IconButton", message: { Align: { axis: "X", aggregate: "Max" } }, tooltip: "Align Right", props: { icon: "AlignRight", size: 24 } },

				{ kind: "Separator", props: { type: "Unrelated" } },

				{ kind: "IconButton", message: { Align: { axis: "Y", aggregate: "Min" } }, tooltip: "Align Top", props: { icon: "AlignTop", size: 24 } },
				{ kind: "IconButton", message: { Align: { axis: "Y", aggregate: "Center" } }, tooltip: "Align Vertical Center", props: { icon: "AlignVerticalCenter", size: 24 } },
				{ kind: "IconButton", message: { Align: { axis: "Y", aggregate: "Max" } }, tooltip: "Align Bottom", props: { icon: "AlignBottom", size: 24 } },

				{ kind: "Separator", props: { type: "Related" } },

				{
					kind: "PopoverButton",
					popover: {
						title: "Align",
						text: "The contents of this popover menu are coming soon",
					},
					props: {},
				},

				{ kind: "Separator", props: { type: "Section" } },

				{ kind: "IconButton", message: "FlipHorizontal", tooltip: "Flip Horizontal", props: { icon: "FlipHorizontal", size: 24 } },
				{ kind: "IconButton", message: "FlipVertical", tooltip: "Flip Vertical", props: { icon: "FlipVertical", size: 24 } },

				{ kind: "Separator", props: { type: "Related" } },

				{
					kind: "PopoverButton",
					popover: {
						title: "Flip",
						text: "The contents of this popover menu are coming soon",
					},
					props: {},
				},

				{ kind: "Separator", props: { type: "Section" } },

				{ kind: "IconButton", tooltip: "Boolean Union", callback: (): void => this.dialog.comingSoon(197), props: { icon: "BooleanUnion", size: 24 } },
				{ kind: "IconButton", tooltip: "Boolean Subtract Front", callback: (): void => this.dialog.comingSoon(197), props: { icon: "BooleanSubtractFront", size: 24 } },
				{ kind: "IconButton", tooltip: "Boolean Subtract Back", callback: (): void => this.dialog.comingSoon(197), props: { icon: "BooleanSubtractBack", size: 24 } },
				{ kind: "IconButton", tooltip: "Boolean Intersect", callback: (): void => this.dialog.comingSoon(197), props: { icon: "BooleanIntersect", size: 24 } },
				{ kind: "IconButton", tooltip: "Boolean Difference", callback: (): void => this.dialog.comingSoon(197), props: { icon: "BooleanDifference", size: 24 } },

				{ kind: "Separator", props: { type: "Related" } },

				{
					kind: "PopoverButton",
					popover: {
						title: "Boolean",
						text: "The contents of this popover menu are coming soon",
					},
					props: {},
				},
			],
			Crop: [],
			Navigate: [],
			Eyedropper: [],
			Text: [{ kind: "NumberInput", optionPath: ["font_size"], props: { min: 1, isInteger: true, unit: " px", label: "Font size" } }],
			Fill: [],
			Gradient: [],
			Brush: [],
			Heal: [],
			Clone: [],
			Patch: [],
			Detail: [],
			Relight: [],
			Path: [],
			Pen: [{ kind: "NumberInput", optionPath: ["weight"], props: { min: 1, isInteger: true, unit: " px", label: "Weight" } }],
			Freehand: [{ kind: "NumberInput", optionPath: ["weight"], props: { min: 1, isInteger: true, unit: " px", label: "Weight" } }],
			Spline: [],
			Line: [{ kind: "NumberInput", optionPath: ["weight"], props: { min: 1, isInteger: true, unit: " px", label: "Weight" } }],
			Rectangle: [],
			Ellipse: [],
			Shape: [{ kind: "NumberInput", optionPath: ["shape_type", "Polygon", "vertices"], props: { min: 3, isInteger: true, label: "Sides" } }],
		};

		return {
			toolOptionsWidgets,
		};
	},
	components: {
		Separator,
		IconButton,
		PopoverButton,
		NumberInput,
		LayoutRow,
	},
});
</script>
