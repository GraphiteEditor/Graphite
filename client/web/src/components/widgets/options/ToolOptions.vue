<template>
	<div class="tool-options">
		<template v-for="(option, index) in optionsMap.get(activeTool) || []" :key="index">
			<IconButton v-if="option.kind === 'icon_button'" :icon="option.icon" :size="24" :title="option.title" :onClick="() => sendToolMessage(option.message)" />
			<Separator v-if="option.kind === 'separator'" :type="option.type" />
			<PopoverButton v-if="option.kind === 'popover_button'">
				<h3>{{ option.title }}</h3>
				<p>{{ option.placeholderText }}</p>
			</PopoverButton>
			<NumberInput v-if="option.kind === 'number_input'" :callback="option.callback" :initialValue="option.initial" :step="option.step" :min="option.min" :updateOnCallback="true" />
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
import Separator, { SeparatorType } from "@/components/widgets/separators/Separator.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";

const wasm = import("@/../wasm/pkg");

type ToolOptionsList = Array<ToolOptions>;
type ToolOptionsMap = Map<string, ToolOptionsList>;
type ToolOptions = IconButtonOption | SeparatorOption | PopoverButtonOption | NumberInputOption;

interface IconButtonOption {
	kind: "icon_button";
	icon: string;
	title: string;
	message?: string;
}

interface SeparatorOption {
	kind: "separator";
	type: SeparatorType;
}

interface PopoverButtonOption {
	kind: "popover_button";
	title: string;
	placeholderText: string;
}

interface NumberInputOption {
	kind: "number_input";
	initial: number;
	step: number;
	min?: number;
	callback?: Function;
}

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
		async sendToolMessage(message?: string) {
			if (message) {
				const { send_tool_message } = await wasm;
				send_tool_message(this.$props.activeTool || "", message);
			}
		},
	},
	data() {
		const optionsMap: ToolOptionsMap = new Map([
			[
				"Select",
				[
					{ kind: "icon_button", icon: "AlignHorizontalLeft", title: "Horizontal Align Left" },
					{ kind: "icon_button", icon: "AlignHorizontalCenter", title: "Horizontal Align Center" },
					{ kind: "icon_button", icon: "AlignHorizontalRight", title: "Horizontal Align Right" },

					{ kind: "separator", type: SeparatorType.Unrelated },

					{ kind: "icon_button", icon: "AlignVerticalTop", title: "Vertical Align Top" },
					{ kind: "icon_button", icon: "AlignVerticalCenter", title: "Vertical Align Center" },
					{ kind: "icon_button", icon: "AlignVerticalBottom", title: "Vertical Align Bottom" },

					{ kind: "separator", type: SeparatorType.Related },

					{ kind: "popover_button", title: "Align", placeholderText: "More alignment-related buttons will be here" },

					{ kind: "separator", type: SeparatorType.Section },

					{ kind: "icon_button", icon: "FlipHorizontal", title: "Flip Horizontal", message: "FlipHorizontal" },
					{ kind: "icon_button", icon: "FlipVertical", title: "Flip Vertical", message: "FlipVertical" },

					{ kind: "separator", type: SeparatorType.Related },

					{ kind: "popover_button", title: "Flip", placeholderText: "More flip-related buttons will be here" },

					{ kind: "separator", type: SeparatorType.Section },

					{ kind: "icon_button", icon: "BooleanUnion", title: "Boolean Union" },
					{ kind: "icon_button", icon: "BooleanSubtractFront", title: "Boolean Subtract Front" },
					{ kind: "icon_button", icon: "BooleanSubtractBack", title: "Boolean Subtract Back" },
					{ kind: "icon_button", icon: "BooleanIntersect", title: "Boolean Intersect" },
					{ kind: "icon_button", icon: "BooleanDifference", title: "Boolean Difference" },

					{ kind: "separator", type: SeparatorType.Related },

					{ kind: "popover_button", title: "Boolean", placeholderText: "More boolean-related buttons will be here" },
				],
			],
			["Shape", [{ kind: "number_input", initial: 6, step: 1, min: 3, callback: this.setToolOptions }]],
		]);

		return {
			optionsMap,
			SeparatorType,
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
