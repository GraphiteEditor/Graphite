export function browserVersion(): string {
	const agent = window.navigator.userAgent;
	let match = agent.match(/(opera|chrome|safari|firefox|msie|trident(?=\/))\/?\s*(\d+)/i) || [];

	if (/trident/i.test(match[1])) {
		const browser = /\brv[ :]+(\d+)/g.exec(agent) || [];
		return `IE ${browser[1] || ""}`.trim();
	}

	if (match[1] === "Chrome") {
		let browser = agent.match(/\bEdg\/(\d+)/);
		if (browser !== null) return `Edge (Chromium) ${browser[1]}`;

		browser = agent.match(/\bOPR\/(\d+)/);
		if (browser !== null) return `Opera ${browser[1]}`;
	}

	match = match[2] ? [match[1], match[2]] : [navigator.appName, navigator.appVersion, "-?"];

	const browser = agent.match(/version\/(\d+)/i);
	if (browser !== null) match.splice(1, 1, browser[1]);

	return `${match[0]} ${match[1]}`;
}

export function operatingSystem(detailed = false): string {
	const osTableDetailed: Record<string, string> = {
		"Windows NT 10": "Windows 10 or 11",
		"Windows NT 6.3": "Windows 8.1",
		"Windows NT 6.2": "Windows 8",
		"Windows NT 6.1": "Windows 7",
		"Windows NT 6.0": "Windows Vista",
		"Windows NT 5.1": "Windows XP",
		"Windows NT 5.0": "Windows 2000",
		Mac: "Mac",
		X11: "Unix",
		Linux: "Linux",
		Unknown: "Unknown",
	};
	const osTableSimple: Record<string, string> = {
		Windows: "Windows",
		Mac: "Mac",
		Linux: "Linux",
		Unknown: "Unknown",
	};
	const osTable = detailed ? osTableDetailed : osTableSimple;

	const userAgentOS = Object.keys(osTable).find((key) => window.navigator.userAgent.includes(key));
	return osTable[userAgentOS || "Unknown"];
}

export function operatingSystemIsMac(): boolean {
	return operatingSystem() === "Mac";
}
