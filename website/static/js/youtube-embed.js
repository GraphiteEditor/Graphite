window.addEventListener("DOMContentLoaded", () => {
	document.querySelectorAll("[data-youtube-embed]").forEach((placeholder) => {
		placeholder.addEventListener("click", () => {
			const videoId = placeholder.attributes["data-youtube-embed"].value;
			const timestamp = placeholder.attributes["data-youtube-timestamp"]?.value
			placeholder.outerHTML = `<iframe width="1280" height="720" src="https://www.youtube.com/embed/${videoId}?${timestamp ? `start=${timestamp}&` : ""}autoplay=1" frameborder="0" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture" allowfullscreen></iframe>`;
		});
	});
});
