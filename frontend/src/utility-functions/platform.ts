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

export type OperatingSystem = "Windows" | "Mac" | "Linux";

export function operatingSystem(): OperatingSystem {
	const osTable: Record<string, OperatingSystem> = { Windows: "Windows", Mac: "Mac", Linux: "Linux" };

	const userAgentOS = Object.keys(osTable).find((key) => window.navigator.userAgent.includes(key));
	return osTable[userAgentOS || "Windows"];
}
