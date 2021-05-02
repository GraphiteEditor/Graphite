<template>
	<LayoutCol :class="'layer-tree-panel'">
		<LayoutRow :class="'options-bar'">
			<NumberInput />
			<NumberInput />
			<DropdownButton />
		</LayoutRow>
		<LayoutRow :class="'layer-tree'">
			<LayoutCol :class="'list'">
				<div
					class="layer-row"
					v-for="layerId in Array(5)
						.fill()
						.map((_, i) => i)"
					:key="layerId"
				>
					<div class="layer-visibility">
						<IconButton v-if="layerId % 2 == 0" @click="hideLayer(layerId)" :size="24" title="Visible"><EyeVisible /></IconButton>
						<IconButton v-if="layerId % 2 == 1" @click="showLayer(layerId)" :size="24" title="Hidden"><EyeHidden /></IconButton>
					</div>
					<div class="layer">
						<div class="layer-thumbnail"></div>
						<div class="layer-type-icon">
							<IconContainer :size="24" title="Path"><NodeTypePath /></IconContainer>
						</div>
						<div class="layer-name">
							<span>Foo bar</span>
						</div>
					</div>
				</div>
			</LayoutCol>
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.layer-tree-panel {
	.options-bar {
		height: 32px;
		margin: 0 8px;
		align-items: center;
	}

	.layer-row {
		display: flex;
		height: 36px;
		align-items: center;
		margin: 0 8px;

		.layer {
			display: flex;
			align-items: center;
			background: #555;
			border-radius: 4px;
			width: 100%;
			height: 100%;
			margin-left: 4px;
			padding-left: 16px;
		}

		& + .layer-row {
			margin-top: 2px;
		}

		.layer-thumbnail {
			width: 64px;
			height: 100%;
			background: white;
		}

		.layer-type-icon {
			margin: 0 8px;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import { ResponseType, registerResponseHandler } from "../../response-handler";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import NumberInput from "../widgets/NumberInput.vue";
import DropdownButton from "../widgets/DropdownButton.vue";
import IconButton from "../widgets/IconButton.vue";
import IconContainer from "../widgets/IconContainer.vue";
import EyeVisible from "../../../assets/svg/24x24-bounds-16x16-icon/visibility-eye-visible.svg";
import EyeHidden from "../../../assets/svg/24x24-bounds-16x16-icon/visibility-eye-hidden.svg";
import NodeTypePath from "../../../assets/svg/24x24-node-type-icon/node-type-path.svg";

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		DropdownButton,
		NumberInput,
		IconButton,
		IconContainer,
		EyeVisible,
		EyeHidden,
		NodeTypePath,
	},
	props: {},
	methods: {
		hideLayer(layerId: number) {
			console.log(`Hidden layer ID: ${layerId}`);
		},
		showLayer(layerId: number) {
			console.log(`Shown layer ID: ${layerId}`);
		},
	},
	mounted() {
		registerResponseHandler(ResponseType["Document::ExpandFolder"], (responseData: Response) => {
			console.log("ExpandFolder: ", responseData);
            const responsePath = responseData.Document.ExpandFolder.path;
            const responseLayers = responseData.Document.ExpandFolder.children;

            if (responsePath.length > 0) console.error("Non root paths are currently not implemented");

            this.layers = responseLayers;
		});
		registerResponseHandler(ResponseType["Document::CollapseFolder"], (responseData) => {
			console.log("CollapseFolder: ", responseData);
		});
	},
	data() {
		return {
            layers: [],
        };
	},
});

type Response =  Document;
type Tool = ToolResponse;
type Document = DocumentResponse;

interface LayerPanelEntry {
	name: string,
	visible: boolean,
	layer_type: LayerType,
}

enum LayerType {
	Folder,
	Shape,
}

type ToolResponse = SetActiveTool | UpdateCanvas;

interface SetActiveTool {
    tool_name: string
}

interface UpdateCanvas {
    document: string
}

type DocumentResponse = DocumentChanged | CollapseFolder | ExpandFolder;

interface DocumentChanged {}
interface CollapseFolder { path: Array<number>}
interface ExpandFolder { path: Array<number>, children: Array<LayerPanelEntry>}
</script>
