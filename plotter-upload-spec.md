# Spec: "Send to Plotter" — upload the current document as SVG to a LAN print server

## What this is

We are running a conference booth where visitors draw vector artwork in Graphite at demo
stations, and their drawing gets physically plotted with a pen on letter paper by a Cricut
machine as a takeaway. A small HTTP print server (a separate, already-finished project) drives
the Cricut; it accepts SVG jobs over the LAN and manages a queue with a live dashboard.

The missing piece, and the task for you, is on the Graphite side: an in-app action that exports
the current document as SVG and POSTs it to that print server. Today we have to save an SVG file
and manually upload it via the dashboard; visitors should instead be able to click one thing
inside Graphite.

## Booth context (why the design looks like this)

- Four Windows 11 demo stations run Graphite. The booth LAN is an isolated switch with **no
  internet**: static IPs on `192.168.77.0/24` (stations are `.10`, `.20`, `.30`, `.40`), blank
  gateway. Because there is no internet, each station runs Graphite from a **local dev server**
  (`npm start`, i.e. an `http://localhost` origin), not from the production website.
- The print server runs on the station connected to the Cricut, listening on port `4747`. At
  the booth it is expected at `http://192.168.77.10:4747`, but the host may change, so the
  endpoint URL must be user-configurable and persisted. For development, the natural default is
  `http://localhost:4747`.
- Sending a job is safe and non-destructive: the queue is human-gated twice (a booth attendant
  resumes the paused queue from the dashboard, and each plot additionally waits for a physical
  Go button press on the machine). Graphite only needs to fire the upload and report success or
  failure; queue management, previews, and status all already exist on the server's dashboard.

## The endpoint

`POST http://<host>:4747/api/jobs`

Two accepted body forms; **use the raw form** (simplest):

1. **Raw SVG body** — any `content-type` that is not `application/json` (use
   `image/svg+xml`). The job name is taken from the `?name=` query parameter.
2. JSON body (`content-type: application/json`): `{ "svg": "<svg …>", "name": "…", "options": { … } }`.

Example of the raw form:

```
POST http://192.168.77.10:4747/api/jobs?name=Alices%20drawing
content-type: image/svg+xml

<svg xmlns="http://www.w3.org/2000/svg" …>…</svg>
```

Server behavior:

- **Success**: `201` with JSON `{ "id": "<job id>", "name": "<name>", "status": "queued" }`.
- **Validation failure**: `400` with JSON `{ "error": "request body must contain an <svg>" }`
  (the body must match `/<svg[\s>]/i`). Other errors return `500` with `{ "error": "…" }`.
- **Body limit**: 25 MB.
- **CORS**: fully open. Every response carries `access-control-allow-origin: *`, and `OPTIONS`
  preflights are answered with `access-control-allow-methods: GET,POST,DELETE,OPTIONS` and
  `access-control-allow-headers: content-type` (plus `access-control-allow-private-network:
  true` for Chrome's private-network-access preflight). A plain browser `fetch` works.
- If a `name` is omitted the server invents one; still, always send the document name so the
  attendant can tell jobs apart on the dashboard.
- Do **not** send job options from Graphite (paper size, rotation, etc. exist as query
  parameters, but the booth-wide defaults are configured on the server; the client staying
  dumb is a feature).

## What the plotter does with the SVG (sets expectations for the export)

- The artwork is auto-scaled to fit 7.5×10" (letter paper minus margin) preserving aspect
  ratio, and auto-rotated to portrait when clearly landscape. Absolute units and document size
  in the SVG are irrelevant; only the aspect ratio and the shapes matter.
- Everything is drawn with a **pen**: every path renders as its outline. Fills are not filled
  in; a filled shape plots as its contour.
- The server already strips Graphite's artboard background: an exported artboard produces a
  background `<rect>` immediately before a `<g clip-path="url(#artboard-…)">` group, and the
  server removes exactly that rect. So exporting a document with an artboard is fine as-is.
  Known limitation: a solid background drawn as anything else (a giant `<polyline>`, a path)
  is NOT stripped and would be plotted, but that is a server concern, not yours.
- Use the same SVG serialization as the existing file export (File > Export); the server is
  known to handle that output. Do not invent a new export path.

## What to build in Graphite

1. A menu action (e.g. **File > Send to Plotter…**, near Export) that opens a small dialog:
   - **Server address** text field, persisted across sessions (default `http://localhost:4747`).
     Accept a bare `host:port` or full origin; normalize to `<origin>/api/jobs` internally.
   - **Job name** text field, defaulting to the document name.
   - A **Send** button.
2. On send: export the current document to an SVG string exactly as File > Export SVG would
   (whole document / all artboards, default settings), then
   `fetch(endpoint, { method: 'POST', headers: { 'content-type': 'image/svg+xml' }, body: svg })`
   with the name in the query string.
3. Feedback:
   - Success (`201`): a brief confirmation (e.g. a toast/dialog: "Sent to plotter queue as
     '<name>'").
   - Failure: show the reason. Distinguish "could not reach the server" (network error —
     wrong address, server not running) from a server-reported error (`400`/`500` JSON
     `error` field). Booth attendants are non-experts under time pressure; the message should
     say what to check ("Is the print server running at <address>?").
   - A pending state on the button while in flight; sends should not be double-fireable.
4. Non-goals: no queue status display, no job management, no auth, no retry logic, no
   settings beyond the address field. The server dashboard covers all of that.

## Testing without the plotter

The print server is a zero-dependency Node ≥ 20 project; run `node bin/cricut-print-server.mjs
serve` from its repo and it listens on `:4747` even with no Cricut hardware or Design Space
running — submitted jobs simply sit in the paused queue, visible with previews at
`http://127.0.0.1:4747/`. That is the ideal end-to-end check: send from Graphite, see the job
card appear with the right name and a correct preview.

If you don't have that repo, a sufficient mock is:

```js
require('node:http').createServer((req, res) => {
  let b = '';
  req.on('data', (c) => (b += c));
  req.on('end', () => {
    const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type', 'access-control-allow-methods': 'GET,POST,DELETE,OPTIONS', 'access-control-allow-private-network': 'true' };
    if (req.method === 'OPTIONS') { res.writeHead(204, cors); return res.end(); }
    const ok = /<svg[\s>]/i.test(b);
    res.writeHead(ok ? 201 : 400, { ...cors, 'content-type': 'application/json' });
    res.end(JSON.stringify(ok ? { id: 'test1', name: new URL(req.url, 'http://x').searchParams.get('name'), status: 'queued' } : { error: 'request body must contain an <svg>' }));
    console.log(req.method, req.url, b.length, 'bytes');
  });
}).listen(4747);
```

## Gotchas

- **Mixed content**: a Graphite instance served over HTTPS (the production site) cannot
  `fetch` an `http://` LAN address — the browser blocks it silently-ish. This is fine for the
  booth (stations run `http://localhost` dev builds, and localhost is a secure context allowed
  to reach private hosts), but don't be confused if a test from the production site fails;
  consider mentioning it in the failure message if `location.protocol === 'https:'` and the
  target is `http:`.
- **Chrome private network access**: Chrome sends a special preflight when a page reaches into
  a private network. The server answers it (`access-control-allow-private-network: true`), so
  this should just work; noted here in case a future Chrome version tightens behavior.
- The SVG can be large (procedural documents); it is sent as one POST body, well under the
  25 MB limit in practice. No chunking or compression needed.
