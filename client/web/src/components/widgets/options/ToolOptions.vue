<template>
	<div class="tool-options">
		<template v-for="(option, index) in optionsMap.get(activeTool) || []" :key="index">
			<IconButton v-if="option.kind === 'IconButton'" :icon="option.icon" :size="24" :title="option.title" :onClick="() => sendToolMessage(option.message)" />
			<Separator v-if="option.kind === 'Separator'" :type="option.type" />
			<PopoverButton v-if="option.kind === 'PopoverButton'">
				<h3>{{ option.title }}</h3>
				<p>{{ option.placeholderText }}</p>
			</PopoverButton>
			<NumberInput v-if="option.kind === 'NumberInput'" :callback="option.callback" :initialValue="option.initial" :step="option.step" :min="option.min" :updateOnCallback="true" />
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
	kind: "IconButton";
	icon: string;
	title: string;
	message?: string;
}

interface SeparatorOption {
	kind: "Separator";
	type: SeparatorType;
}

interface PopoverButtonOption {
	kind: "PopoverButton";
	title: string;
	placeholderText: string;
}

interface NumberInputOption {
	kind: "NumberInput";
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
					{ kind: "IconButton", icon: "AlignLeft", title: "Align Left", message: "AlignLeft" },
					{ kind: "IconButton", icon: "AlignHorizontalCenter", title: "Align Horizontal Center", message: "AlignHorizontalCenter" },
					{ kind: "IconButton", icon: "AlignRight", title: "Align Right", message: "AlignRight" },

					{ kind: "Separator", type: SeparatorType.Unrelated },

					{ kind: "IconButton", icon: "AlignTop", title: "Align Top", message: "AlignTop" },
					{ kind: "IconButton", icon: "AlignVerticalCenter", title: "Align Vertical Center", message: "AlignVerticalCenter" },
					{ kind: "IconButton", icon: "AlignBottom", title: "Align Bottom", message: "AlignBottom" },

					{ kind: "Separator", type: SeparatorType.Related },

					{ kind: "PopoverButton", title: "Align", placeholderText: "More alignment-related buttons will be here" },

					{ kind: "Separator", type: SeparatorType.Section },

					{ kind: "IconButton", icon: "FlipHorizontal", title: "Flip Horizontal", message: "FlipHorizontal" },
					{ kind: "IconButton", icon: "FlipVertical", title: "Flip Vertical", message: "FlipVertical" },

					{ kind: "Separator", type: SeparatorType.Related },

					{ kind: "PopoverButton", title: "Flip", placeholderText: "More flip-related buttons will be here" },

					{ kind: "Separator", type: SeparatorType.Section },

					{ kind: "IconButton", icon: "BooleanUnion", title: "Boolean Union" },
					{ kind: "IconButton", icon: "BooleanSubtractFront", title: "Boolean Subtract Front" },
					{ kind: "IconButton", icon: "BooleanSubtractBack", title: "Boolean Subtract Back" },
					{ kind: "IconButton", icon: "BooleanIntersect", title: "Boolean Intersect" },
					{ kind: "IconButton", icon: "BooleanDifference", title: "Boolean Difference" },

					{ kind: "Separator", type: SeparatorType.Related },

					{ kind: "PopoverButton", title: "Boolean", placeholderText: "More boolean-related buttons will be here" },
				],
			],
			["Shape", [{ kind: "NumberInput", initial: 6, step: 1, min: 3, callback: this.setToolOptions }]],
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
