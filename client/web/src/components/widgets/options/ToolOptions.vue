<template>
	<div class="tool-options">
		<template v-for="(option, index) in optionsMap.get(activeTool) || []" :key="index">
			<IconButton v-if="option.kind === 'icon'" :icon="option.icon" :size="24" :title="option.title" />
			<Separator v-if="option.kind === 'separator'" :type="option.type" />
			<PopoverButton v-if="option.kind === 'popover'">
				<h3>{{ option.title }}</h3>
				<p>{{ option.placeholder_text }}</p>
			</PopoverButton>
			<NumberInput v-if="option.kind === 'number'" :callback="option.callback" :initialValue="option.initial" :step="option.step" :min="option.min" :updateOnCallback="true" />
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

type ToolOptions = IconOption | SeparatorOption | PopoverOption | NumberOption;

interface IconOption {
	kind: "icon";
	icon: string;
	title: string;
}

interface SeparatorOption {
	kind: "separator";
	type: SeparatorType;
}

interface PopoverOption {
	kind: "popover";
	title: string;
	placeholder_text: string;
}

interface NumberOption {
	kind: "number";
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
		async setToolOptions(new_value: number) {
			// TODO: Each value-input widget (i.e. not a button) should map to a field in an options struct,
			// and updating a widget should send the whole updated struct to the backend.
			// Later, it could send a single-field update to the backend.
			const { set_tool_options } = await wasm;
			set_tool_options(this.$props.activeTool || "Select", { Shape: { shape_type: { Polygon: { vertices: new_value } } } });
		},
	},
	data() {
		return {
			optionsMap: new Map([
				[
					"Select",
					[
						{ kind: "icon", icon: "AlignHorizontalLeft", title: "Horizontal Align Left" },
						{ kind: "icon", icon: "AlignHorizontalCenter", title: "Horizontal Align Center" },
						{ kind: "icon", icon: "AlignHorizontalRight", title: "Horizontal Align Right" },

						{ kind: "separator", type: SeparatorType.Unrelated },

						{ kind: "icon", icon: "AlignVerticalTop", title: "Vertical Align Top" },
						{ kind: "icon", icon: "AlignVerticalCenter", title: "Vertical Align Center" },
						{ kind: "icon", icon: "AlignVerticalBottom", title: "Vertical Align Bottom" },

						{ kind: "separator", type: SeparatorType.Related },

						{ kind: "popover", title: "Align", placeholder_text: "More alignment-related buttons will be here" },

						{ kind: "separator", type: SeparatorType.Section },

						{ kind: "icon", icon: "FlipHorizontal", title: "Flip Horizontal" },
						{ kind: "icon", icon: "FlipVertical", title: "Flip Vertical" },

						{ kind: "separator", type: SeparatorType.Related },

						{ kind: "popover", title: "Flip", placeholder_text: "More flip-related buttons will be here" },

						{ kind: "separator", type: SeparatorType.Section },

						{ kind: "icon", icon: "BooleanUnion", title: "Boolean Union" },
						{ kind: "icon", icon: "BooleanSubtractFront", title: "Boolean Subtract Front" },
						{ kind: "icon", icon: "BooleanSubtractBack", title: "Boolean Subtract Back" },
						{ kind: "icon", icon: "BooleanIntersect", title: "Boolean Intersect" },
						{ kind: "icon", icon: "BooleanDifference", title: "Boolean Difference" },

						{ kind: "separator", type: SeparatorType.Related },

						{ kind: "popover", title: "Boolean", placeholder_text: "More boolean-related buttons will be here" },
					],
				],
				["Shape", [{ kind: "number", initial: 6, step: 1, min: 3, callback: this.setToolOptions }]],
			]) as ToolOptionsMap,
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
