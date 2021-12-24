<template>
	<div class="status-bar">
		<template v-for="(hintGroup, index) in hintData" :key="hintGroup">
			<Separator :type="SeparatorType.Section" v-if="index !== 0" />
			<template v-for="hint in hintGroup" :key="hint">
				<span v-if="hint.plus" class="plus">+</span>
				<UserInputLabel :inputMouse="hint.mouse" :inputKeys="hint.key_groups">{{ hint.label }}</UserInputLabel>
			</template>
		</template>
	</div>
</template>

<style lang="scss">
.status-bar {
	display: flex;
	height: 24px;
	margin: 0 -4px;

	.separator.section {
		margin: 0;
	}

	.plus {
		display: flex;
		align-items: center;
		font-weight: 700;
	}

	.user-input-label + .user-input-label {
		margin-left: 0;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { SeparatorType } from "@/components/widgets/widgets";

import UserInputLabel from "@/components/widgets/labels/UserInputLabel.vue";
import Separator from "@/components/widgets/separators/Separator.vue";
import { HintData, UpdateInputHints } from "@/dispatcher/js-messages";

export default defineComponent({
	inject: ["editor"],
	components: {
		UserInputLabel,
		Separator,
	},
	data() {
		return {
			SeparatorType,
			hintData: [] as HintData,
		};
	},
	mounted() {
		this.editor.dispatcher.subscribeJsMessage(UpdateInputHints, (updateInputHints) => {
			this.hintData = updateInputHints.hint_data;
		});

		// Switch away from, and back to, the Select Tool to make it display the correct hints in the status bar
		this.editor.instance.select_tool("Path");
		this.editor.instance.select_tool("Select");
	},
});
</script>
