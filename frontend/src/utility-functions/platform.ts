import { isPlatformNative } from "@graphite/../wasm/pkg/graphite_wasm";

export function browserVersion(): string {
	const agent = window.navigator.userAgent;
	let match = agent.match(/(opera|chrome|safari|firefox|msie|trident(?=\/))\/?\s*(\d+)/i) || [];

	if (/trident/i.test(match[1])) {
		const browser = /\brv[ :]+(\d+)/g.exec(agent) || [];
		return `IE ${browser[1] || ""}`.trim();
	}

	if (match[1] === "Chrome") {
		let browser = agent.match(/\bEdg\/(\d+)/) || undefined;
		if (browser !== undefined) return `Edge (Chromium) ${browser[1]}`;

		browser = agent.match(/\bOPR\/(\d+)/) || undefined;
		if (browser !== undefined) return `Opera ${browser[1]}`;
	}

	match = match[2] ? [match[1], match[2]] : [navigator.appName, navigator.appVersion, "-?"];

	const browser = agent.match(/version\/(\d+)/i) || undefined;
	if (browser !== undefined) match.splice(1, 1, browser[1]);

	return `${match[0]} ${match[1]}`;
}

export type OperatingSystem = "Windows" | "Mac" | "Linux" | "Unknown";

export function operatingSystem(): OperatingSystem {
	const osTable: Record<string, OperatingSystem> = {
		Windows: "Windows",
		Mac: "Mac",
		Linux: "Linux",
		Unknown: "Unknown",
	};

	const userAgentOS = Object.keys(osTable).find((key) => window.navigator.userAgent.includes(key));
	return osTable[userAgentOS || "Unknown"];
}

export function isDesktop(): boolean {
	return isPlatformNative();
}

export function isEventSupported(eventName: string) {
	const onEventName = `on${eventName}`;

	let tag = "div";
	if (["select", "change"].includes(eventName)) tag = "select";
	if (["submit", "reset"].includes(eventName)) tag = "form";
	if (["error", "load", "abort"].includes(eventName)) tag = "img";
	const element = document.createElement(tag);

	if (onEventName in element) return true;

	// Check if "return;" gets converted into a function, meaning the event is supported
	element.setAttribute(eventName, "return;");
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	return typeof (element as Record<string, any>)[onEventName] === "function";
}
