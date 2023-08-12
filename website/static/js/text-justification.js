window.addEventListener("DOMContentLoaded", () => {
	document.querySelectorAll("section p").forEach((paragraph) => {
		// Recursively traverse the DOM tree and modify the text nodes
		const recursivelyAddWbr = (node) => {
			if (node.nodeType === Node.TEXT_NODE) {
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
		
		// Perform the recursive traversal and replace the text nodes
		const mutationQueue = [];
		recursivelyAddWbr(paragraph);
		mutationQueue.forEach(([node, newNodes]) => {
			node.replaceWith(...newNodes);
		});
	});
});
