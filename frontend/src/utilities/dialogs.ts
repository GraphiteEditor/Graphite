import { subscribeJsMessage } from "@/utilities/js-message-dispatcher";
import { DisplayAboutGraphiteDialog } from "@/utilities/js-messages";
import { createDialog } from "@/utilities/dialog";
import { TextButtonWidget } from "@/components/widgets/widgets";

subscribeJsMessage(DisplayAboutGraphiteDialog, () => {
	const date = new Date(process.env.VUE_APP_COMMIT_DATE || "");
	const dateString = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
	const timeString = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
	const timezoneName = Intl.DateTimeFormat(undefined, { timeZoneName: "long" })
		.formatToParts(new Date())
		.find((part) => part.type === "timeZoneName");
	const timezoneNameString = timezoneName && timezoneName.value;

	const hash = (process.env.VUE_APP_COMMIT_HASH || "").substring(0, 12);

	const details = `
Release Series: ${process.env.VUE_APP_RELEASE_SERIES}

Date: ${dateString} ${timeString} ${timezoneNameString}
Hash: ${hash}
Branch: ${process.env.VUE_APP_COMMIT_BRANCH}
`.trim();

	const buttons: TextButtonWidget[] = [
		{
			kind: "TextButton",
			callback: () => window.open("https://www.graphite.design", "_blank"),
			props: { label: "Website", emphasized: false, minWidth: 0 },
		},
		{
			kind: "TextButton",
			callback: () => window.open("https://github.com/GraphiteEditor/Graphite/graphs/contributors", "_blank"),
			props: { label: "Credits", emphasized: false, minWidth: 0 },
		},
		{
			kind: "TextButton",
			callback: () => window.open("https://raw.githubusercontent.com/GraphiteEditor/Graphite/master/LICENSE.txt", "_blank"),
			props: { label: "License", emphasized: false, minWidth: 0 },
		},
		{
			kind: "TextButton",
			callback: () => window.open("/third-party-licenses.txt", "_blank"),
			props: { label: "Third-Party Licenses", emphasized: false, minWidth: 0 },
		},
	];

	createDialog("GraphiteLogo", "Graphite", details, buttons);
});
