<script lang="ts">
import type { JsMessageType } from "$lib/../glue/messages"
import { onMount } from "svelte"

let graphiteVersion= "i don't know"
onMount(async () => {

  let { JsEditorHandle } = await import("$lib/shim")
  const editor = new JsEditorHandle((messageType: JsMessageType, messageData: Record<string, unknown>): void => {
	console.log("message:", messageType, messageData)	
	});
  graphiteVersion = editor.graphiteDocumentVersion()
  
})
</script>

<h1>Welcome to SvelteKit</h1>
<p>
  Visit <a href="https://kit.svelte.dev">kit.svelte.dev</a> to read the documentation
</p>
<p>
  Graphite version: {graphiteVersion}
</p>