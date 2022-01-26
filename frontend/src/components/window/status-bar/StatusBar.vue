<template>
	<LayoutRow class="status-bar">
		<LayoutRow class="hint-groups">
			<template v-for="(hintGroup, index) in hintData" :key="hintGroup">
				<Separator :type="'Section'" v-if="index !== 0" />
				<template v-for="hint in hintGroup" :key="hint">
					<LayoutRow v-if="hint.plus" class="plus">+</LayoutRow>
					<UserInputLabel :inputMouse="hint.mouse" :inputKeys="hint.key_groups">{{ hint.label }}</UserInputLabel>
				</template>
			</template>
		</LayoutRow>
	</LayoutRow>
</template>

<style lang="scss">
.status-bar {
	height: 24px;
	width: 100%;
	flex: 0 0 auto;

	.hint-groups {
		flex: 0 0 auto;
		max-width: 100%;
		margin: 0 -4px;
		overflow: hidden;

		.separator.section {
			margin: 0;
		}

		.plus {
			flex: 0 0 auto;
			align-items: center;
			font-weight: 700;
		}

		.user-input-label + .user-input-label {
			margin-left: 0;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { HintData, UpdateInputHints } from "@/dispatcher/js-messages";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import UserInputLabel from "@/components/widgets/labels/UserInputLabel.vue";
import Separator from "@/components/widgets/separators/Separator.vue";

export default defineComponent({
	inject: ["editor"],
	data() {
		return {
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
	components: {
		UserInputLabel,
		Separator,
		LayoutRow,
	},
});
</script>
