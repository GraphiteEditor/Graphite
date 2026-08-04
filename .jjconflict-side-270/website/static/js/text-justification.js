window.addEventListener("DOMContentLoaded", () => {
	document.querySelectorAll("section p").forEach((paragraph) => {
		const /** @type {[Text, NodeList][]} */ mutationQueue = [];

		// Recursively traverse the DOM tree and modify the text nodes
		const recursivelyAddWbr = (/** @type {ChildNode} **/ node) => {
			if (node.nodeType === Node.TEXT_NODE) {
				if (!(node instanceof Text)) return;

				const newNodes = node.textContent.split("/");
				for (let i = 0; i < newNodes.length - 1; i++) {
					newNodes[i] += "/";
				}

				const tempSpan = document.createElement("span");
				tempSpan.innerHTML = newNodes.join("<wbr>");
				const replacementNodes = tempSpan.childNodes;

				mutationQueue.push([node, replacementNodes]);
			} else {
				node.childNodes.forEach(recursivelyAddWbr);
			}
		};

		// Perform the recursive traversal
		recursivelyAddWbr(paragraph);

		// Replace the text nodes
		mutationQueue.forEach(([node, newNodes]) => node.replaceWith(...newNodes));
	});
});
