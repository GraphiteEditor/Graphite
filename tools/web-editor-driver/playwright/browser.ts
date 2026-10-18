// The part of the web editor driver that needs Playwright: it owns the driver's own Chromium and carries out the basic steps
// the Rust session sends it, read as one JSON request per line on stdin and answered with one JSON line each on stdout

import readline from "node:readline";
import { setTimeout as sleep } from "node:timers/promises";
import { chromium } from "playwright-core";
import type { Browser, BrowserContext, Locator, Page } from "playwright-core";

const MAX_CONSOLE_ERRORS = 50;

type Point = [number, number];
type Box = { x: number; y: number; width: number; height: number };
type MouseButton = "left" | "middle" | "right";

type Request =
	| { type: "launch"; url: string; size: Point; scale: number; headed: boolean; readySelector: string; timeout: number }
	| { type: "close" }
	| { type: "resize"; size: Point }
	| { type: "mouseMove"; to: Point }
	| { type: "mouseDown"; button: MouseButton; clickCount: number }
	| { type: "mouseUp"; button: MouseButton; clickCount: number }
	| { type: "wheel"; delta: Point }
	| { type: "keyDown"; key: string }
	| { type: "keyUp"; key: string }
	| { type: "type"; text: string }
	| { type: "screenshot"; clip?: Box }
	| { type: "boundingBox"; selector: string }
	| { type: "locate"; selector?: string; text?: string; fieldSelector: string }
	| { type: "nextFrame"; timeout: number }
	| { type: "takeEvents" };

type Envelope = { id: number; request: Request };

let browser: Browser | undefined;
let context: BrowserContext | undefined;
let page: Page | undefined;
let consoleErrors: string[] = [];
let crashed = false;

function recordError(message: string) {
	if (consoleErrors.length < MAX_CONSOLE_ERRORS) consoleErrors.push(message);
}

async function launch(request: Extract<Request, { type: "launch" }>) {
	if (browser) throw new Error("The browser is already open");

	// The full browser in its new headless mode behaves like the one a person uses, unlike the lighter headless shell
	browser = await chromium.launch({ channel: "chromium", headless: !request.headed });
	context = await browser.newContext({ viewport: { width: request.size[0], height: request.size[1] }, deviceScaleFactor: request.scale });
	page = await context.newPage();

	page.on("console", (message) => message.type() === "error" && recordError(message.text()));
	page.on("pageerror", (error) => recordError(String(error)));
	page.on("crash", () => (crashed = true));

	await page.goto(request.url, { waitUntil: "domcontentloaded", timeout: request.timeout });
	await page.waitForSelector(request.readySelector, { timeout: request.timeout });
}

async function close() {
	await context?.close();
	await browser?.close();
}

type Located = { box: Box; text: string; value?: string };

function elementsWith(current: Page, selector: string | undefined, text: string | undefined): Locator {
	if (selector === undefined) {
		if (text === undefined) throw new Error("Locating needs a selector or some text");
		return current.getByText(text);
	}

	const matching = current.locator(selector);
	return text === undefined ? matching : matching.filter({ hasText: text });
}

async function locate(current: Page, request: Extract<Request, { type: "locate" }>): Promise<Located[]> {
	const located: Located[] = [];
	const elements = await elementsWith(current, request.selector, request.text).all();
	for (let i = 0; i < elements.length; i++) {
		const box = await elements[i].boundingBox();
		if (!box) continue;

		// What the element holds if it is a field, or else what the first field inside it holds
		const value = await elements[i].evaluate((element, fieldSelector) => {
			const field = element.matches(fieldSelector) ? element : element.querySelector(fieldSelector);
			return field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement ? field.value : undefined;
		}, request.fieldSelector);
		located.push({ box, text: (await elements[i].textContent()) || "", value });
	}
	return located;
}

async function perform(request: Request): Promise<unknown> {
	if (request.type === "launch") return launch(request);
	if (request.type === "close") return close();

	const current = page;
	if (!current) throw new Error("The browser is not open");

	switch (request.type) {
		case "resize":
			return current.setViewportSize({ width: request.size[0], height: request.size[1] });
		case "mouseMove":
			return current.mouse.move(request.to[0], request.to[1]);
		case "mouseDown":
			return current.mouse.down({ button: request.button, clickCount: request.clickCount });
		case "mouseUp":
			return current.mouse.up({ button: request.button, clickCount: request.clickCount });
		case "wheel":
			return current.mouse.wheel(request.delta[0], request.delta[1]);
		case "keyDown":
			return current.keyboard.down(request.key);
		case "keyUp":
			return current.keyboard.up(request.key);
		case "type":
			return current.keyboard.type(request.text);
		case "screenshot":
			return (await current.screenshot(request.clip ? { clip: request.clip } : {})).toString("base64");
		case "boundingBox": {
			const locator = current.locator(request.selector);
			if ((await locator.count()) === 0) return undefined;
			return locator.first().boundingBox();
		}
		case "locate":
			return locate(current, request);
		case "nextFrame": {
			// Gives up after the timeout, so a page that has stopped drawing cannot hold up every later request
			const frame = current.evaluate(() => new Promise((resolve) => requestAnimationFrame(resolve)));
			await Promise.race([frame, sleep(request.timeout)]);
			return undefined;
		}
		case "takeEvents": {
			const events = { consoleErrors, crashed };
			consoleErrors = [];
			return events;
		}
	}
}

// Resolves once the reply is written, so exiting afterward cannot cut it off
function reply(message: { id: number; value?: unknown; error?: string }): Promise<void> {
	return new Promise((resolve) => process.stdout.write(`${JSON.stringify(message)}\n`, () => resolve()));
}

// Requests are carried out as they arrive, so closing can interrupt a step that is still running
const lines = readline.createInterface({ input: process.stdin });
lines.on("line", (line) => {
	let id = 0;
	const handle = async () => {
		const envelope: Envelope = JSON.parse(line);
		id = envelope.id;

		await reply({ id, value: await perform(envelope.request) });
		if (envelope.request.type === "close") process.exit(0);
	};
	handle().catch((error) => reply({ id, error: error instanceof Error ? error.message : String(error) }));
});

// The browser has no reason to stay open once the session that sends the requests is gone
lines.on("close", () => close().finally(() => process.exit(0)));
