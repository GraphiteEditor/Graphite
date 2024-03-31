// vite.config.ts
import { spawnSync } from "child_process";
import path from "path";
import { svelte } from "file:///home/abundance/open_source/Graphite/frontend/node_modules/@sveltejs/vite-plugin-svelte/src/index.js";
import rollupPluginLicense from "file:///home/abundance/open_source/Graphite/frontend/node_modules/rollup-plugin-license/dist/index.js";
import { sveltePreprocess } from "file:///home/abundance/open_source/Graphite/frontend/node_modules/svelte-preprocess/dist/autoProcess.js";
import { defineConfig } from "file:///home/abundance/open_source/Graphite/frontend/node_modules/vite/dist/node/index.js";
import { default as viteMultipleAssets } from "file:///home/abundance/open_source/Graphite/frontend/node_modules/vite-multiple-assets/dist/index.mjs";
var __vite_injected_original_dirname = "/home/abundance/open_source/Graphite/frontend";
var projectRootDir = path.resolve(__vite_injected_original_dirname);
var ALLOWED_LICENSES = [
  "Apache-2.0 WITH LLVM-exception",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "ISC",
  "MIT-0",
  "MIT",
  "MPL-2.0",
  "OpenSSL",
  "Unicode-DFS-2016",
  "Zlib"
];
var vite_config_default = defineConfig({
  plugins: [
    svelte({
      preprocess: [sveltePreprocess()],
      onwarn(warning, defaultHandler) {
        const suppressed = ["css-unused-selector", "vite-plugin-svelte-css-no-scopable-elements", "a11y-no-static-element-interactions", "a11y-no-noninteractive-element-interactions"];
        if (suppressed.includes(warning.code))
          return;
        defaultHandler?.(warning);
      }
    }),
    viteMultipleAssets(["../demo-artwork"])
  ],
  resolve: {
    alias: [
      { find: /@graphite-frontend\/(.*\.svg)/, replacement: path.resolve(projectRootDir, "$1?raw") },
      { find: "@graphite-frontend", replacement: projectRootDir },
      { find: "@graphite/../assets", replacement: path.resolve(projectRootDir, "assets") },
      { find: "@graphite/../public", replacement: path.resolve(projectRootDir, "public") },
      { find: "@graphite", replacement: path.resolve(projectRootDir, "src") }
    ]
  },
  server: {
    port: 8080,
    host: "0.0.0.0"
  },
  build: {
    rollupOptions: {
      plugins: [
        rollupPluginLicense({
          thirdParty: {
            allow: {
              test: `(${ALLOWED_LICENSES.join(" OR ")})`,
              failOnUnlicensed: true,
              failOnViolation: true
            },
            output: {
              file: path.resolve(__vite_injected_original_dirname, "./dist/third-party-licenses.txt"),
              template: formatThirdPartyLicenses
            }
          }
        })
      ],
      output: {
        // Inject `.min` into the filename of minified CSS files to tell Cloudflare not to minify it again.
        // Cloudflare's minifier breaks the CSS due to a bug where it removes whitespace around calc() plus operators.
        assetFileNames: (info) => `assets/[name]-[hash]${info.name?.endsWith(".css") ? ".min" : ""}[extname]`
      }
    }
  }
});
function formatThirdPartyLicenses(jsLicenses) {
  let licenses = generateRustLicenses() || [];
  if (licenses.length === 0) {
    console.error("Could not run `cargo about`, which is required to generate license information.");
    console.error("To install cargo-about on your system, you can run `cargo install cargo-about`.");
    console.error("License information is required in production builds. Aborting.");
    process.exit(1);
  }
  if (jsLicenses.length === 0) {
    console.error("No JavaScript package licenses were found by `rollup-plugin-license`. Please investigate.");
    console.error("License information is required in production builds. Aborting.");
    process.exit(1);
  }
  jsLicenses.forEach((jsLicense) => {
    const name = jsLicense.name || "";
    const version = jsLicense.version || "";
    const author = jsLicense.author?.text() || "";
    const licenseText = trimBlankLines(jsLicense.licenseText ?? "");
    const licenseName = jsLicense.license || "";
    let repository = jsLicense.repository || "";
    if (repository && typeof repository === "object")
      repository = repository.url;
    const repo = repository ? repository.replace(/^.*(github.com\/.*?\/.*?)(?:.git)/, "https://$1") : repository;
    const matchedLicense = licenses.find((license) => trimBlankLines(license.licenseText || "") === licenseText);
    const packages = { name, version, author, repository: repo };
    if (matchedLicense)
      matchedLicense.packages.push(packages);
    else
      licenses.push({ licenseName, licenseText, packages: [packages] });
  });
  licenses.forEach((license, licenseIndex) => {
    licenses.slice(0, licenseIndex).forEach((comparisonLicense) => {
      if (license.licenseText === comparisonLicense.licenseText) {
        license.packages.push(...comparisonLicense.packages);
        comparisonLicense.packages = [];
      }
    });
  });
  licenses = licenses.filter((license) => {
    license.packages = license.packages.filter(
      (packageInfo) => !(packageInfo.repository && packageInfo.repository.toLowerCase().includes("github.com/GraphiteEditor/Graphite".toLowerCase())) && !(packageInfo.author && packageInfo.author.toLowerCase().includes("contact@graphite.rs"))
    );
    return license.packages.length > 0;
  });
  licenses.sort((a, b) => a.licenseName.localeCompare(b.licenseName));
  licenses.sort((a, b) => a.licenseText.localeCompare(b.licenseText));
  licenses.forEach((license) => {
    license.packages.sort((a, b) => a.name.localeCompare(b.name));
  });
  let formattedLicenseNotice = "GRAPHITE THIRD-PARTY SOFTWARE LICENSE NOTICES";
  licenses.forEach((license) => {
    let packagesWithSameLicense = "";
    license.packages.forEach((packageInfo) => {
      const { name, version, author, repository } = packageInfo;
      packagesWithSameLicense += `${name} ${version}${author ? ` - ${author}` : ""}${repository ? ` - ${repository}` : ""}
`;
    });
    packagesWithSameLicense = packagesWithSameLicense.trim();
    const packagesLineLength = Math.max(...packagesWithSameLicense.split("\n").map((line) => line.length));
    formattedLicenseNotice += "\n\n--------------------------------------------------------------------------------\n\n";
    formattedLicenseNotice += `The following packages are licensed under the terms of the ${license.licenseName} license as printed beneath:
`;
    formattedLicenseNotice += `${"_".repeat(packagesLineLength)}
`;
    formattedLicenseNotice += `${packagesWithSameLicense}
`;
    formattedLicenseNotice += `${"\u203E".repeat(packagesLineLength)}
`;
    formattedLicenseNotice += `${license.licenseText}
`;
  });
  return formattedLicenseNotice;
}
function generateRustLicenses() {
  console.info("\n\nGenerating license information for Rust code\n");
  try {
    const { stdout, stderr, status } = spawnSync("cargo", ["about", "generate", "about.hbs"], {
      cwd: path.join(__vite_injected_original_dirname, ".."),
      encoding: "utf8",
      timeout: 6e4,
      // One minute
      shell: true,
      windowsHide: true
      // Hide the terminal on Windows
    });
    if (status !== 0) {
      if (status !== 101) {
        console.error("cargo-about failed", status, stderr);
      }
      return void 0;
    }
    if (!stdout.trim().startsWith("GENERATED_BY_CARGO_ABOUT:")) {
      console.error("Unexpected output from cargo-about", stdout);
      return void 0;
    }
    const indirectEval = eval;
    const licensesArray = indirectEval(stdout);
    const rustLicenses = (licensesArray || []).map(
      (rustLicense) => ({
        licenseName: htmlDecode(rustLicense.licenseName),
        licenseText: trimBlankLines(htmlDecode(rustLicense.licenseText)),
        packages: rustLicense.packages.map(
          (packageInfo) => ({
            name: htmlDecode(packageInfo.name),
            version: htmlDecode(packageInfo.version),
            author: htmlDecode(packageInfo.author).replace(/\[(.*), \]/, "$1").replace("[]", ""),
            repository: htmlDecode(packageInfo.repository)
          })
        )
      })
    );
    return rustLicenses;
  } catch (_) {
    return void 0;
  }
}
function htmlDecode(input) {
  if (!input)
    return input;
  const htmlEntities = {
    nbsp: " ",
    copy: "\xA9",
    reg: "\xAE",
    lt: "<",
    gt: ">",
    amp: "&",
    apos: "'",
    quot: `"`
  };
  return input.replace(/&([^;]+);/g, (entity, entityCode) => {
    const maybeEntity = Object.keys(htmlEntities).find((key) => key === entityCode);
    if (maybeEntity) {
      return maybeEntity[1];
    }
    let match;
    if (match = entityCode.match(/^#x([\da-fA-F]+)$/)) {
      return String.fromCharCode(parseInt(match[1], 16));
    }
    if (match = entityCode.match(/^#(\d+)$/)) {
      return String.fromCharCode(~~match[1]);
    }
    return entity;
  });
}
function trimBlankLines(input) {
  let result = input.replace(/\r/g, "");
  while (result.charAt(0) === "\r" || result.charAt(0) === "\n") {
    result = result.slice(1);
  }
  while (result.slice(-1) === "\r" || result.slice(-1) === "\n") {
    result = result.slice(0, -1);
  }
  return result;
}
export {
  vite_config_default as default
};
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcudHMiXSwKICAic291cmNlc0NvbnRlbnQiOiBbImNvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9kaXJuYW1lID0gXCIvaG9tZS9hYnVuZGFuY2Uvb3Blbl9zb3VyY2UvR3JhcGhpdGUvZnJvbnRlbmRcIjtjb25zdCBfX3ZpdGVfaW5qZWN0ZWRfb3JpZ2luYWxfZmlsZW5hbWUgPSBcIi9ob21lL2FidW5kYW5jZS9vcGVuX3NvdXJjZS9HcmFwaGl0ZS9mcm9udGVuZC92aXRlLmNvbmZpZy50c1wiO2NvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9pbXBvcnRfbWV0YV91cmwgPSBcImZpbGU6Ly8vaG9tZS9hYnVuZGFuY2Uvb3Blbl9zb3VyY2UvR3JhcGhpdGUvZnJvbnRlbmQvdml0ZS5jb25maWcudHNcIjsvKiBlc2xpbnQtZGlzYWJsZSBuby1jb25zb2xlICovXG5cbmltcG9ydCB7IHNwYXduU3luYyB9IGZyb20gXCJjaGlsZF9wcm9jZXNzXCI7XG5cbmltcG9ydCBwYXRoIGZyb20gXCJwYXRoXCI7XG5cbmltcG9ydCB7IHN2ZWx0ZSB9IGZyb20gXCJAc3ZlbHRlanMvdml0ZS1wbHVnaW4tc3ZlbHRlXCI7XG5pbXBvcnQgcm9sbHVwUGx1Z2luTGljZW5zZSwgeyB0eXBlIERlcGVuZGVuY3kgfSBmcm9tIFwicm9sbHVwLXBsdWdpbi1saWNlbnNlXCI7XG5pbXBvcnQgeyBzdmVsdGVQcmVwcm9jZXNzIH0gZnJvbSBcInN2ZWx0ZS1wcmVwcm9jZXNzL2Rpc3QvYXV0b1Byb2Nlc3NcIjtcbmltcG9ydCB7IGRlZmluZUNvbmZpZyB9IGZyb20gXCJ2aXRlXCI7XG5pbXBvcnQgeyBkZWZhdWx0IGFzIHZpdGVNdWx0aXBsZUFzc2V0cyB9IGZyb20gXCJ2aXRlLW11bHRpcGxlLWFzc2V0c1wiO1xuXG5jb25zdCBwcm9qZWN0Um9vdERpciA9IHBhdGgucmVzb2x2ZShfX2Rpcm5hbWUpO1xuXG4vLyBLZWVwIHRoaXMgbGlzdCBpbiBzeW5jIHdpdGggdGhvc2UgaW4gYC9hYm91dC50b21sYCBhbmQgYC9kZW55LnRvbWxgLlxuY29uc3QgQUxMT1dFRF9MSUNFTlNFUyA9IFtcblx0XCJBcGFjaGUtMi4wIFdJVEggTExWTS1leGNlcHRpb25cIixcblx0XCJBcGFjaGUtMi4wXCIsXG5cdFwiQlNELTItQ2xhdXNlXCIsXG5cdFwiQlNELTMtQ2xhdXNlXCIsXG5cdFwiQlNMLTEuMFwiLFxuXHRcIkNDMC0xLjBcIixcblx0XCJJU0NcIixcblx0XCJNSVQtMFwiLFxuXHRcIk1JVFwiLFxuXHRcIk1QTC0yLjBcIixcblx0XCJPcGVuU1NMXCIsXG5cdFwiVW5pY29kZS1ERlMtMjAxNlwiLFxuXHRcIlpsaWJcIixcbl07XG5cbi8vIGh0dHBzOi8vdml0ZWpzLmRldi9jb25maWcvXG5leHBvcnQgZGVmYXVsdCBkZWZpbmVDb25maWcoe1xuXHRwbHVnaW5zOiBbXG5cdFx0c3ZlbHRlKHtcblx0XHRcdHByZXByb2Nlc3M6IFtzdmVsdGVQcmVwcm9jZXNzKCldLFxuXHRcdFx0b253YXJuKHdhcm5pbmcsIGRlZmF1bHRIYW5kbGVyKSB7XG5cdFx0XHRcdGNvbnN0IHN1cHByZXNzZWQgPSBbXCJjc3MtdW51c2VkLXNlbGVjdG9yXCIsIFwidml0ZS1wbHVnaW4tc3ZlbHRlLWNzcy1uby1zY29wYWJsZS1lbGVtZW50c1wiLCBcImExMXktbm8tc3RhdGljLWVsZW1lbnQtaW50ZXJhY3Rpb25zXCIsIFwiYTExeS1uby1ub25pbnRlcmFjdGl2ZS1lbGVtZW50LWludGVyYWN0aW9uc1wiXTtcblx0XHRcdFx0aWYgKHN1cHByZXNzZWQuaW5jbHVkZXMod2FybmluZy5jb2RlKSkgcmV0dXJuO1xuXG5cdFx0XHRcdGRlZmF1bHRIYW5kbGVyPy4od2FybmluZyk7XG5cdFx0XHR9LFxuXHRcdH0pLFxuXHRcdHZpdGVNdWx0aXBsZUFzc2V0cyhbXCIuLi9kZW1vLWFydHdvcmtcIl0pLFxuXHRdLFxuXHRyZXNvbHZlOiB7XG5cdFx0YWxpYXM6IFtcblx0XHRcdHsgZmluZDogL0BncmFwaGl0ZS1mcm9udGVuZFxcLyguKlxcLnN2ZykvLCByZXBsYWNlbWVudDogcGF0aC5yZXNvbHZlKHByb2plY3RSb290RGlyLCBcIiQxP3Jhd1wiKSB9LFxuXHRcdFx0eyBmaW5kOiBcIkBncmFwaGl0ZS1mcm9udGVuZFwiLCByZXBsYWNlbWVudDogcHJvamVjdFJvb3REaXIgfSxcblx0XHRcdHsgZmluZDogXCJAZ3JhcGhpdGUvLi4vYXNzZXRzXCIsIHJlcGxhY2VtZW50OiBwYXRoLnJlc29sdmUocHJvamVjdFJvb3REaXIsIFwiYXNzZXRzXCIpIH0sXG5cdFx0XHR7IGZpbmQ6IFwiQGdyYXBoaXRlLy4uL3B1YmxpY1wiLCByZXBsYWNlbWVudDogcGF0aC5yZXNvbHZlKHByb2plY3RSb290RGlyLCBcInB1YmxpY1wiKSB9LFxuXHRcdFx0eyBmaW5kOiBcIkBncmFwaGl0ZVwiLCByZXBsYWNlbWVudDogcGF0aC5yZXNvbHZlKHByb2plY3RSb290RGlyLCBcInNyY1wiKSB9LFxuXHRcdF0sXG5cdH0sXG5cdHNlcnZlcjoge1xuXHRcdHBvcnQ6IDgwODAsXG5cdFx0aG9zdDogXCIwLjAuMC4wXCIsXG5cdH0sXG5cdGJ1aWxkOiB7XG5cdFx0cm9sbHVwT3B0aW9uczoge1xuXHRcdFx0cGx1Z2luczogW1xuXHRcdFx0XHRyb2xsdXBQbHVnaW5MaWNlbnNlKHtcblx0XHRcdFx0XHR0aGlyZFBhcnR5OiB7XG5cdFx0XHRcdFx0XHRhbGxvdzoge1xuXHRcdFx0XHRcdFx0XHR0ZXN0OiBgKCR7QUxMT1dFRF9MSUNFTlNFUy5qb2luKFwiIE9SIFwiKX0pYCxcblx0XHRcdFx0XHRcdFx0ZmFpbE9uVW5saWNlbnNlZDogdHJ1ZSxcblx0XHRcdFx0XHRcdFx0ZmFpbE9uVmlvbGF0aW9uOiB0cnVlLFxuXHRcdFx0XHRcdFx0fSxcblx0XHRcdFx0XHRcdG91dHB1dDoge1xuXHRcdFx0XHRcdFx0XHRmaWxlOiBwYXRoLnJlc29sdmUoX19kaXJuYW1lLCBcIi4vZGlzdC90aGlyZC1wYXJ0eS1saWNlbnNlcy50eHRcIiksXG5cdFx0XHRcdFx0XHRcdHRlbXBsYXRlOiBmb3JtYXRUaGlyZFBhcnR5TGljZW5zZXMsXG5cdFx0XHRcdFx0XHR9LFxuXHRcdFx0XHRcdH0sXG5cdFx0XHRcdH0pLFxuXHRcdFx0XSxcblx0XHRcdG91dHB1dDoge1xuXHRcdFx0XHQvLyBJbmplY3QgYC5taW5gIGludG8gdGhlIGZpbGVuYW1lIG9mIG1pbmlmaWVkIENTUyBmaWxlcyB0byB0ZWxsIENsb3VkZmxhcmUgbm90IHRvIG1pbmlmeSBpdCBhZ2Fpbi5cblx0XHRcdFx0Ly8gQ2xvdWRmbGFyZSdzIG1pbmlmaWVyIGJyZWFrcyB0aGUgQ1NTIGR1ZSB0byBhIGJ1ZyB3aGVyZSBpdCByZW1vdmVzIHdoaXRlc3BhY2UgYXJvdW5kIGNhbGMoKSBwbHVzIG9wZXJhdG9ycy5cblx0XHRcdFx0YXNzZXRGaWxlTmFtZXM6IChpbmZvKSA9PiBgYXNzZXRzL1tuYW1lXS1baGFzaF0ke2luZm8ubmFtZT8uZW5kc1dpdGgoXCIuY3NzXCIpID8gXCIubWluXCIgOiBcIlwifVtleHRuYW1lXWAsXG5cdFx0XHR9LFxuXHRcdH0sXG5cdH0sXG59KTtcblxudHlwZSBMaWNlbnNlSW5mbyA9IHtcblx0bGljZW5zZU5hbWU6IHN0cmluZztcblx0bGljZW5zZVRleHQ6IHN0cmluZztcblx0cGFja2FnZXM6IFBhY2thZ2VJbmZvW107XG59O1xuXG50eXBlIFBhY2thZ2VJbmZvID0ge1xuXHRuYW1lOiBzdHJpbmc7XG5cdHZlcnNpb246IHN0cmluZztcblx0YXV0aG9yOiBzdHJpbmc7XG5cdHJlcG9zaXRvcnk6IHN0cmluZztcbn07XG5cbmZ1bmN0aW9uIGZvcm1hdFRoaXJkUGFydHlMaWNlbnNlcyhqc0xpY2Vuc2VzOiBEZXBlbmRlbmN5W10pOiBzdHJpbmcge1xuXHQvLyBHZW5lcmF0ZSB0aGUgUnVzdCBsaWNlbnNlIGluZm9ybWF0aW9uLlxuXHRsZXQgbGljZW5zZXMgPSBnZW5lcmF0ZVJ1c3RMaWNlbnNlcygpIHx8IFtdO1xuXG5cdC8vIEVuc3VyZSB3ZSBoYXZlIGxpY2Vuc2UgaW5mb3JtYXRpb24gdG8gd29yayB3aXRoIGJlZm9yZSBwcm9jZWVkaW5nLlxuXHRpZiAobGljZW5zZXMubGVuZ3RoID09PSAwKSB7XG5cdFx0Ly8gVGhpcyBpcyBwcm9iYWJseSBjYXVzZWQgYnkgYGNhcmdvIGFib3V0YCBub3QgYmVpbmcgaW5zdGFsbGVkLlxuXHRcdGNvbnNvbGUuZXJyb3IoXCJDb3VsZCBub3QgcnVuIGBjYXJnbyBhYm91dGAsIHdoaWNoIGlzIHJlcXVpcmVkIHRvIGdlbmVyYXRlIGxpY2Vuc2UgaW5mb3JtYXRpb24uXCIpO1xuXHRcdGNvbnNvbGUuZXJyb3IoXCJUbyBpbnN0YWxsIGNhcmdvLWFib3V0IG9uIHlvdXIgc3lzdGVtLCB5b3UgY2FuIHJ1biBgY2FyZ28gaW5zdGFsbCBjYXJnby1hYm91dGAuXCIpO1xuXHRcdGNvbnNvbGUuZXJyb3IoXCJMaWNlbnNlIGluZm9ybWF0aW9uIGlzIHJlcXVpcmVkIGluIHByb2R1Y3Rpb24gYnVpbGRzLiBBYm9ydGluZy5cIik7XG5cblx0XHRwcm9jZXNzLmV4aXQoMSk7XG5cdH1cblx0aWYgKGpzTGljZW5zZXMubGVuZ3RoID09PSAwKSB7XG5cdFx0Y29uc29sZS5lcnJvcihcIk5vIEphdmFTY3JpcHQgcGFja2FnZSBsaWNlbnNlcyB3ZXJlIGZvdW5kIGJ5IGByb2xsdXAtcGx1Z2luLWxpY2Vuc2VgLiBQbGVhc2UgaW52ZXN0aWdhdGUuXCIpO1xuXHRcdGNvbnNvbGUuZXJyb3IoXCJMaWNlbnNlIGluZm9ybWF0aW9uIGlzIHJlcXVpcmVkIGluIHByb2R1Y3Rpb24gYnVpbGRzLiBBYm9ydGluZy5cIik7XG5cblx0XHRwcm9jZXNzLmV4aXQoMSk7XG5cdH1cblxuXHQvLyBBdWdtZW50IHRoZSBpbXBvcnRlZCBSdXN0IGxpY2Vuc2UgbGlzdCB3aXRoIHRoZSBwcm92aWRlZCBKUyBsaWNlbnNlIGxpc3QuXG5cdGpzTGljZW5zZXMuZm9yRWFjaCgoanNMaWNlbnNlKSA9PiB7XG5cdFx0Y29uc3QgbmFtZSA9IGpzTGljZW5zZS5uYW1lIHx8IFwiXCI7XG5cdFx0Y29uc3QgdmVyc2lvbiA9IGpzTGljZW5zZS52ZXJzaW9uIHx8IFwiXCI7XG5cdFx0Y29uc3QgYXV0aG9yID0ganNMaWNlbnNlLmF1dGhvcj8udGV4dCgpIHx8IFwiXCI7XG5cdFx0Y29uc3QgbGljZW5zZVRleHQgPSB0cmltQmxhbmtMaW5lcyhqc0xpY2Vuc2UubGljZW5zZVRleHQgPz8gXCJcIik7XG5cdFx0Y29uc3QgbGljZW5zZU5hbWUgPSBqc0xpY2Vuc2UubGljZW5zZSB8fCBcIlwiO1xuXHRcdGxldCByZXBvc2l0b3J5ID0ganNMaWNlbnNlLnJlcG9zaXRvcnkgfHwgXCJcIjtcblx0XHRpZiAocmVwb3NpdG9yeSAmJiB0eXBlb2YgcmVwb3NpdG9yeSA9PT0gXCJvYmplY3RcIikgcmVwb3NpdG9yeSA9IHJlcG9zaXRvcnkudXJsO1xuXG5cdFx0Ly8gUmVtb3ZlIHRoZSBgZ2l0K2Agb3IgYGdpdDovL2AgcHJlZml4IGFuZCBgLmdpdGAgc3VmZml4LlxuXHRcdGNvbnN0IHJlcG8gPSByZXBvc2l0b3J5ID8gcmVwb3NpdG9yeS5yZXBsYWNlKC9eLiooZ2l0aHViLmNvbVxcLy4qP1xcLy4qPykoPzouZ2l0KS8sIFwiaHR0cHM6Ly8kMVwiKSA6IHJlcG9zaXRvcnk7XG5cblx0XHRjb25zdCBtYXRjaGVkTGljZW5zZSA9IGxpY2Vuc2VzLmZpbmQoKGxpY2Vuc2UpID0+IHRyaW1CbGFua0xpbmVzKGxpY2Vuc2UubGljZW5zZVRleHQgfHwgXCJcIikgPT09IGxpY2Vuc2VUZXh0KTtcblxuXHRcdGNvbnN0IHBhY2thZ2VzOiBQYWNrYWdlSW5mbyA9IHsgbmFtZSwgdmVyc2lvbiwgYXV0aG9yLCByZXBvc2l0b3J5OiByZXBvIH07XG5cdFx0aWYgKG1hdGNoZWRMaWNlbnNlKSBtYXRjaGVkTGljZW5zZS5wYWNrYWdlcy5wdXNoKHBhY2thZ2VzKTtcblx0XHRlbHNlIGxpY2Vuc2VzLnB1c2goeyBsaWNlbnNlTmFtZSwgbGljZW5zZVRleHQsIHBhY2thZ2VzOiBbcGFja2FnZXNdIH0pO1xuXHR9KTtcblxuXHQvLyBEZS1kdXBsaWNhdGUgYW55IGxpY2Vuc2VzIHdpdGggdGhlIHNhbWUgdGV4dCBieSBtZXJnaW5nIHRoZWlyIGxpc3RzIG9mIHBhY2thZ2VzLlxuXHRsaWNlbnNlcy5mb3JFYWNoKChsaWNlbnNlLCBsaWNlbnNlSW5kZXgpID0+IHtcblx0XHRsaWNlbnNlcy5zbGljZSgwLCBsaWNlbnNlSW5kZXgpLmZvckVhY2goKGNvbXBhcmlzb25MaWNlbnNlKSA9PiB7XG5cdFx0XHRpZiAobGljZW5zZS5saWNlbnNlVGV4dCA9PT0gY29tcGFyaXNvbkxpY2Vuc2UubGljZW5zZVRleHQpIHtcblx0XHRcdFx0bGljZW5zZS5wYWNrYWdlcy5wdXNoKC4uLmNvbXBhcmlzb25MaWNlbnNlLnBhY2thZ2VzKTtcblx0XHRcdFx0Y29tcGFyaXNvbkxpY2Vuc2UucGFja2FnZXMgPSBbXTtcblx0XHRcdFx0Ly8gQWZ0ZXIgZW1wdHlpbmcgdGhlIHBhY2thZ2VzLCB0aGUgcmVkdW5kYW50IGxpY2Vuc2Ugd2l0aCBubyBwYWNrYWdlcyB3aWxsIGJlIHJlbW92ZWQgaW4gdGhlIG5leHQgc3RlcCdzIGBmaWx0ZXIoKWAuXG5cdFx0XHR9XG5cdFx0fSk7XG5cdH0pO1xuXG5cdC8vIEZpbHRlciBvdXQgdGhlIGludGVybmFsIEdyYXBoaXRlIGNyYXRlcywgd2hpY2ggYXJlIG5vdCB0aGlyZC1wYXJ0eS5cblx0bGljZW5zZXMgPSBsaWNlbnNlcy5maWx0ZXIoKGxpY2Vuc2UpID0+IHtcblx0XHRsaWNlbnNlLnBhY2thZ2VzID0gbGljZW5zZS5wYWNrYWdlcy5maWx0ZXIoXG5cdFx0XHQocGFja2FnZUluZm8pID0+XG5cdFx0XHRcdCEocGFja2FnZUluZm8ucmVwb3NpdG9yeSAmJiBwYWNrYWdlSW5mby5yZXBvc2l0b3J5LnRvTG93ZXJDYXNlKCkuaW5jbHVkZXMoXCJnaXRodWIuY29tL0dyYXBoaXRlRWRpdG9yL0dyYXBoaXRlXCIudG9Mb3dlckNhc2UoKSkpICYmXG5cdFx0XHRcdCEocGFja2FnZUluZm8uYXV0aG9yICYmIHBhY2thZ2VJbmZvLmF1dGhvci50b0xvd2VyQ2FzZSgpLmluY2x1ZGVzKFwiY29udGFjdEBncmFwaGl0ZS5yc1wiKSksXG5cdFx0KTtcblx0XHRyZXR1cm4gbGljZW5zZS5wYWNrYWdlcy5sZW5ndGggPiAwO1xuXHR9KTtcblxuXHQvLyBTb3J0IHRoZSBsaWNlbnNlcywgYW5kIHRoZSBwYWNrYWdlcyB1c2luZyBlYWNoIGxpY2Vuc2UsIGFscGhhYmV0aWNhbGx5LlxuXHRsaWNlbnNlcy5zb3J0KChhLCBiKSA9PiBhLmxpY2Vuc2VOYW1lLmxvY2FsZUNvbXBhcmUoYi5saWNlbnNlTmFtZSkpO1xuXHRsaWNlbnNlcy5zb3J0KChhLCBiKSA9PiBhLmxpY2Vuc2VUZXh0LmxvY2FsZUNvbXBhcmUoYi5saWNlbnNlVGV4dCkpO1xuXHRsaWNlbnNlcy5mb3JFYWNoKChsaWNlbnNlKSA9PiB7XG5cdFx0bGljZW5zZS5wYWNrYWdlcy5zb3J0KChhLCBiKSA9PiBhLm5hbWUubG9jYWxlQ29tcGFyZShiLm5hbWUpKTtcblx0fSk7XG5cblx0Ly8gQXBwZW5kIGEgYmxvY2sgZm9yIGVhY2ggbGljZW5zZSBzaGFyZWQgYnkgbXVsdGlwbGUgcGFja2FnZXMgd2l0aCBpZGVudGljYWwgbGljZW5zZSB0ZXh0LlxuXHRsZXQgZm9ybWF0dGVkTGljZW5zZU5vdGljZSA9IFwiR1JBUEhJVEUgVEhJUkQtUEFSVFkgU09GVFdBUkUgTElDRU5TRSBOT1RJQ0VTXCI7XG5cdGxpY2Vuc2VzLmZvckVhY2goKGxpY2Vuc2UpID0+IHtcblx0XHRsZXQgcGFja2FnZXNXaXRoU2FtZUxpY2Vuc2UgPSBcIlwiO1xuXHRcdGxpY2Vuc2UucGFja2FnZXMuZm9yRWFjaCgocGFja2FnZUluZm8pID0+IHtcblx0XHRcdGNvbnN0IHsgbmFtZSwgdmVyc2lvbiwgYXV0aG9yLCByZXBvc2l0b3J5IH0gPSBwYWNrYWdlSW5mbztcblx0XHRcdHBhY2thZ2VzV2l0aFNhbWVMaWNlbnNlICs9IGAke25hbWV9ICR7dmVyc2lvbn0ke2F1dGhvciA/IGAgLSAke2F1dGhvcn1gIDogXCJcIn0ke3JlcG9zaXRvcnkgPyBgIC0gJHtyZXBvc2l0b3J5fWAgOiBcIlwifVxcbmA7XG5cdFx0fSk7XG5cdFx0cGFja2FnZXNXaXRoU2FtZUxpY2Vuc2UgPSBwYWNrYWdlc1dpdGhTYW1lTGljZW5zZS50cmltKCk7XG5cdFx0Y29uc3QgcGFja2FnZXNMaW5lTGVuZ3RoID0gTWF0aC5tYXgoLi4ucGFja2FnZXNXaXRoU2FtZUxpY2Vuc2Uuc3BsaXQoXCJcXG5cIikubWFwKChsaW5lKSA9PiBsaW5lLmxlbmd0aCkpO1xuXG5cdFx0Zm9ybWF0dGVkTGljZW5zZU5vdGljZSArPSBcIlxcblxcbi0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tLS0tXFxuXFxuXCI7XG5cdFx0Zm9ybWF0dGVkTGljZW5zZU5vdGljZSArPSBgVGhlIGZvbGxvd2luZyBwYWNrYWdlcyBhcmUgbGljZW5zZWQgdW5kZXIgdGhlIHRlcm1zIG9mIHRoZSAke2xpY2Vuc2UubGljZW5zZU5hbWV9IGxpY2Vuc2UgYXMgcHJpbnRlZCBiZW5lYXRoOlxcbmA7XG5cdFx0Zm9ybWF0dGVkTGljZW5zZU5vdGljZSArPSBgJHtcIl9cIi5yZXBlYXQocGFja2FnZXNMaW5lTGVuZ3RoKX1cXG5gO1xuXHRcdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gYCR7cGFja2FnZXNXaXRoU2FtZUxpY2Vuc2V9XFxuYDtcblx0XHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IGAke1wiXHUyMDNFXCIucmVwZWF0KHBhY2thZ2VzTGluZUxlbmd0aCl9XFxuYDtcblx0XHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IGAke2xpY2Vuc2UubGljZW5zZVRleHR9XFxuYDtcblx0fSk7XG5cdHJldHVybiBmb3JtYXR0ZWRMaWNlbnNlTm90aWNlO1xufVxuXG5mdW5jdGlvbiBnZW5lcmF0ZVJ1c3RMaWNlbnNlcygpOiBMaWNlbnNlSW5mb1tdIHwgdW5kZWZpbmVkIHtcblx0Ly8gTG9nIHRoZSBzdGFydGluZyBzdGF0dXMgdG8gdGhlIGJ1aWxkIG91dHB1dC5cblx0Y29uc29sZS5pbmZvKFwiXFxuXFxuR2VuZXJhdGluZyBsaWNlbnNlIGluZm9ybWF0aW9uIGZvciBSdXN0IGNvZGVcXG5cIik7XG5cblx0dHJ5IHtcblx0XHQvLyBDYWxsIGBjYXJnbyBhYm91dGAgaW4gdGhlIHRlcm1pbmFsIHRvIGdlbmVyYXRlIHRoZSBsaWNlbnNlIGluZm9ybWF0aW9uIGZvciBSdXN0IGNyYXRlcy5cblx0XHQvLyBUaGUgYGFib3V0Lmhic2AgZmlsZSBpcyB3cml0dGVuIHNvIGl0IGdlbmVyYXRlcyBhIHZhbGlkIEphdmFTY3JpcHQgYXJyYXkgZXhwcmVzc2lvbiB3aGljaCB3ZSBldmFsdWF0ZSBiZWxvdy5cblx0XHRjb25zdCB7IHN0ZG91dCwgc3RkZXJyLCBzdGF0dXMgfSA9IHNwYXduU3luYyhcImNhcmdvXCIsIFtcImFib3V0XCIsIFwiZ2VuZXJhdGVcIiwgXCJhYm91dC5oYnNcIl0sIHtcblx0XHRcdGN3ZDogcGF0aC5qb2luKF9fZGlybmFtZSwgXCIuLlwiKSxcblx0XHRcdGVuY29kaW5nOiBcInV0ZjhcIixcblx0XHRcdHRpbWVvdXQ6IDYwMDAwLCAvLyBPbmUgbWludXRlXG5cdFx0XHRzaGVsbDogdHJ1ZSxcblx0XHRcdHdpbmRvd3NIaWRlOiB0cnVlLCAvLyBIaWRlIHRoZSB0ZXJtaW5hbCBvbiBXaW5kb3dzXG5cdFx0fSk7XG5cblx0XHQvLyBJZiB0aGUgY29tbWFuZCBmYWlsZWQsIHByaW50IHRoZSBlcnJvciBtZXNzYWdlIGFuZCBleGl0IGVhcmx5LlxuXHRcdGlmIChzdGF0dXMgIT09IDApIHtcblx0XHRcdC8vIENhcmdvIHJldHVybnMgMTAxIHdoZW4gdGhlIHN1YmNvbW1hbmQgKGBhYm91dGApIHdhc24ndCBmb3VuZCwgc28gd2Ugc2tpcCBwcmludGluZyB0aGUgYmVsb3cgZXJyb3IgbWVzc2FnZSBpbiB0aGF0IGNhc2UuXG5cdFx0XHRpZiAoc3RhdHVzICE9PSAxMDEpIHtcblx0XHRcdFx0Y29uc29sZS5lcnJvcihcImNhcmdvLWFib3V0IGZhaWxlZFwiLCBzdGF0dXMsIHN0ZGVycik7XG5cdFx0XHR9XG5cdFx0XHRyZXR1cm4gdW5kZWZpbmVkO1xuXHRcdH1cblxuXHRcdC8vIE1ha2Ugc3VyZSB0aGUgb3V0cHV0IHN0YXJ0cyB3aXRoIHRoaXMgZXhwZWN0ZWQgbGFiZWwsIHdoaWNoIGxldHMgdXMga25vdyB0aGUgZmlsZSBnZW5lcmF0ZWQgd2l0aCBleHBlY3RlZCBvdXRwdXQuXG5cdFx0Ly8gV2UgZG9uJ3Qgd2FudCB0byBldmFsIGFuIGVycm9yIG1lc3NhZ2Ugb3Igc29tZXRoaW5nIGVsc2UsIHNvIHdlIGZhaWwgZWFybHkgaWYgdGhhdCBoYXBwZW5zLlxuXHRcdGlmICghc3Rkb3V0LnRyaW0oKS5zdGFydHNXaXRoKFwiR0VORVJBVEVEX0JZX0NBUkdPX0FCT1VUOlwiKSkge1xuXHRcdFx0Y29uc29sZS5lcnJvcihcIlVuZXhwZWN0ZWQgb3V0cHV0IGZyb20gY2FyZ28tYWJvdXRcIiwgc3Rkb3V0KTtcblx0XHRcdHJldHVybiB1bmRlZmluZWQ7XG5cdFx0fVxuXG5cdFx0Ly8gQ29udmVydCB0aGUgYXJyYXkgSlMgc3ludGF4IHN0cmluZyBpbnRvIGFuIGFjdHVhbCBKUyBhcnJheSBpbiBtZW1vcnkuXG5cdFx0Ly8gU2VjdXJpdHktd2lzZSwgZXZhbCgpIGlzbid0IGFueSB3b3JzZSB0aGFuIHJlcXVpcmUoKSwgYnV0IGl0J3MgYWJsZSB0byB3b3JrIHdpdGhvdXQgYSB0ZW1wb3JhcnkgZmlsZS5cblx0XHQvLyBXZSBjYWxsIGV2YWwgaW5kaXJlY3RseSB0byBhdm9pZCBhIHdhcm5pbmcgYXMgZXhwbGFpbmVkIGhlcmU6IDxodHRwczovL2VzYnVpbGQuZ2l0aHViLmlvL2NvbnRlbnQtdHlwZXMvI2RpcmVjdC1ldmFsPi5cblx0XHRjb25zdCBpbmRpcmVjdEV2YWwgPSBldmFsO1xuXHRcdGNvbnN0IGxpY2Vuc2VzQXJyYXkgPSBpbmRpcmVjdEV2YWwoc3Rkb3V0KSBhcyBMaWNlbnNlSW5mb1tdO1xuXG5cdFx0Ly8gUmVtb3ZlIHRoZSBIVE1MIGNoYXJhY3RlciBlbmNvZGluZyBjYXVzZWQgYnkgSGFuZGxlYmFycy5cblx0XHRjb25zdCBydXN0TGljZW5zZXMgPSAobGljZW5zZXNBcnJheSB8fCBbXSkubWFwKFxuXHRcdFx0KHJ1c3RMaWNlbnNlKTogTGljZW5zZUluZm8gPT4gKHtcblx0XHRcdFx0bGljZW5zZU5hbWU6IGh0bWxEZWNvZGUocnVzdExpY2Vuc2UubGljZW5zZU5hbWUpLFxuXHRcdFx0XHRsaWNlbnNlVGV4dDogdHJpbUJsYW5rTGluZXMoaHRtbERlY29kZShydXN0TGljZW5zZS5saWNlbnNlVGV4dCkpLFxuXHRcdFx0XHRwYWNrYWdlczogcnVzdExpY2Vuc2UucGFja2FnZXMubWFwKFxuXHRcdFx0XHRcdChwYWNrYWdlSW5mbyk6IFBhY2thZ2VJbmZvID0+ICh7XG5cdFx0XHRcdFx0XHRuYW1lOiBodG1sRGVjb2RlKHBhY2thZ2VJbmZvLm5hbWUpLFxuXHRcdFx0XHRcdFx0dmVyc2lvbjogaHRtbERlY29kZShwYWNrYWdlSW5mby52ZXJzaW9uKSxcblx0XHRcdFx0XHRcdGF1dGhvcjogaHRtbERlY29kZShwYWNrYWdlSW5mby5hdXRob3IpXG5cdFx0XHRcdFx0XHRcdC5yZXBsYWNlKC9cXFsoLiopLCBcXF0vLCBcIiQxXCIpXG5cdFx0XHRcdFx0XHRcdC5yZXBsYWNlKFwiW11cIiwgXCJcIiksXG5cdFx0XHRcdFx0XHRyZXBvc2l0b3J5OiBodG1sRGVjb2RlKHBhY2thZ2VJbmZvLnJlcG9zaXRvcnkpLFxuXHRcdFx0XHRcdH0pLFxuXHRcdFx0XHQpLFxuXHRcdFx0fSksXG5cdFx0KTtcblxuXHRcdHJldHVybiBydXN0TGljZW5zZXM7XG5cdH0gY2F0Y2ggKF8pIHtcblx0XHRyZXR1cm4gdW5kZWZpbmVkO1xuXHR9XG59XG5cbmZ1bmN0aW9uIGh0bWxEZWNvZGUoaW5wdXQ6IHN0cmluZyk6IHN0cmluZyB7XG5cdGlmICghaW5wdXQpIHJldHVybiBpbnB1dDtcblxuXHRjb25zdCBodG1sRW50aXRpZXMgPSB7XG5cdFx0bmJzcDogXCIgXCIsXG5cdFx0Y29weTogXCJcdTAwQTlcIixcblx0XHRyZWc6IFwiXHUwMEFFXCIsXG5cdFx0bHQ6IFwiPFwiLFxuXHRcdGd0OiBcIj5cIixcblx0XHRhbXA6IFwiJlwiLFxuXHRcdGFwb3M6IFwiJ1wiLFxuXHRcdHF1b3Q6IGBcImAsXG5cdH07XG5cblx0cmV0dXJuIGlucHV0LnJlcGxhY2UoLyYoW147XSspOy9nLCAoZW50aXR5OiBzdHJpbmcsIGVudGl0eUNvZGU6IHN0cmluZykgPT4ge1xuXHRcdGNvbnN0IG1heWJlRW50aXR5ID0gT2JqZWN0LmtleXMoaHRtbEVudGl0aWVzKS5maW5kKChrZXkpID0+IGtleSA9PT0gZW50aXR5Q29kZSk7XG5cdFx0aWYgKG1heWJlRW50aXR5KSB7XG5cdFx0XHRyZXR1cm4gbWF5YmVFbnRpdHlbMV07XG5cdFx0fVxuXG5cdFx0bGV0IG1hdGNoO1xuXHRcdC8vIGVzbGludC1kaXNhYmxlLW5leHQtbGluZSBuby1jb25kLWFzc2lnblxuXHRcdGlmICgobWF0Y2ggPSBlbnRpdHlDb2RlLm1hdGNoKC9eI3goW1xcZGEtZkEtRl0rKSQvKSkpIHtcblx0XHRcdHJldHVybiBTdHJpbmcuZnJvbUNoYXJDb2RlKHBhcnNlSW50KG1hdGNoWzFdLCAxNikpO1xuXHRcdH1cblx0XHQvLyBlc2xpbnQtZGlzYWJsZS1uZXh0LWxpbmUgbm8tY29uZC1hc3NpZ25cblx0XHRpZiAoKG1hdGNoID0gZW50aXR5Q29kZS5tYXRjaCgvXiMoXFxkKykkLykpKSB7XG5cdFx0XHRyZXR1cm4gU3RyaW5nLmZyb21DaGFyQ29kZSh+fm1hdGNoWzFdKTtcblx0XHR9XG5cdFx0cmV0dXJuIGVudGl0eTtcblx0fSk7XG59XG5cbmZ1bmN0aW9uIHRyaW1CbGFua0xpbmVzKGlucHV0OiBzdHJpbmcpOiBzdHJpbmcge1xuXHRsZXQgcmVzdWx0ID0gaW5wdXQucmVwbGFjZSgvXFxyL2csIFwiXCIpO1xuXG5cdHdoaWxlIChyZXN1bHQuY2hhckF0KDApID09PSBcIlxcclwiIHx8IHJlc3VsdC5jaGFyQXQoMCkgPT09IFwiXFxuXCIpIHtcblx0XHRyZXN1bHQgPSByZXN1bHQuc2xpY2UoMSk7XG5cdH1cblx0d2hpbGUgKHJlc3VsdC5zbGljZSgtMSkgPT09IFwiXFxyXCIgfHwgcmVzdWx0LnNsaWNlKC0xKSA9PT0gXCJcXG5cIikge1xuXHRcdHJlc3VsdCA9IHJlc3VsdC5zbGljZSgwLCAtMSk7XG5cdH1cblxuXHRyZXR1cm4gcmVzdWx0O1xufVxuIl0sCiAgIm1hcHBpbmdzIjogIjtBQUVBLFNBQVMsaUJBQWlCO0FBRTFCLE9BQU8sVUFBVTtBQUVqQixTQUFTLGNBQWM7QUFDdkIsT0FBTyx5QkFBOEM7QUFDckQsU0FBUyx3QkFBd0I7QUFDakMsU0FBUyxvQkFBb0I7QUFDN0IsU0FBUyxXQUFXLDBCQUEwQjtBQVY5QyxJQUFNLG1DQUFtQztBQVl6QyxJQUFNLGlCQUFpQixLQUFLLFFBQVEsZ0NBQVM7QUFHN0MsSUFBTSxtQkFBbUI7QUFBQSxFQUN4QjtBQUFBLEVBQ0E7QUFBQSxFQUNBO0FBQUEsRUFDQTtBQUFBLEVBQ0E7QUFBQSxFQUNBO0FBQUEsRUFDQTtBQUFBLEVBQ0E7QUFBQSxFQUNBO0FBQUEsRUFDQTtBQUFBLEVBQ0E7QUFBQSxFQUNBO0FBQUEsRUFDQTtBQUNEO0FBR0EsSUFBTyxzQkFBUSxhQUFhO0FBQUEsRUFDM0IsU0FBUztBQUFBLElBQ1IsT0FBTztBQUFBLE1BQ04sWUFBWSxDQUFDLGlCQUFpQixDQUFDO0FBQUEsTUFDL0IsT0FBTyxTQUFTLGdCQUFnQjtBQUMvQixjQUFNLGFBQWEsQ0FBQyx1QkFBdUIsK0NBQStDLHVDQUF1Qyw2Q0FBNkM7QUFDOUssWUFBSSxXQUFXLFNBQVMsUUFBUSxJQUFJO0FBQUc7QUFFdkMseUJBQWlCLE9BQU87QUFBQSxNQUN6QjtBQUFBLElBQ0QsQ0FBQztBQUFBLElBQ0QsbUJBQW1CLENBQUMsaUJBQWlCLENBQUM7QUFBQSxF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QUEsWUFDakMsU0FBUyxXQUFXLFlBQVksT0FBTztBQUFBLFlBQ3ZDLFFBQVEsV0FBVyxZQUFZLE1BQU0sRUFDbkMsUUFBUSxjQUFjLElBQUksRUFDMUIsUUFBUSxNQUFNLEVBQUU7QUFBQSxZQUNsQixZQUFZLFdBQVcsWUFBWSxVQUFVO0FBQUEsVUFDOUM7QUFBQSxRQUNEO0FBQUEsTUFDRDtBQUFBLElBQ0Q7QUFFQSxXQUFPO0FBQUEsRUFDUixTQUFTLEdBQUc7QUFDWCxXQUFPO0FBQUEsRUFDUjtBQUNEO0FBRUEsU0FBUyxXQUFXLE9BQXVCO0FBQzFDLE1BQUksQ0FBQztBQUFPLFdBQU87QUFFbkIsUUFBTSxlQUFlO0FBQUEsSUFDcEIsTUFBTTtBQUFBLElBQ04sTUFBTTtBQUFBLElBQ04sS0FBSztBQUFBLElBQ0wsSUFBSTtBQUFBLElBQ0osSUFBSTtBQUFBLElBQ0osS0FBSztBQUFBLElBQ0wsTUFBTTtBQUFBLElBQ04sTUFBTTtBQUFBLEVBQ1A7QUFFQSxTQUFPLE1BQU0sUUFBUSxjQUFjLENBQUMsUUFBZ0IsZUFBdUI7QUFDMUUsVUFBTSxjQUFjLE9BQU8sS0FBSyxZQUFZLEVBQUUsS0FBSyxDQUFDLFFBQVEsUUFBUSxVQUFVO0FBQzlFLFFBQUksYUFBYTtBQUNoQixhQUFPLFlBQVksQ0FBQztBQUFBLElBQ3JCO0FBRUEsUUFBSTtBQUVKLFFBQUssUUFBUSxXQUFXLE1BQU0sbUJBQW1CLEdBQUk7QUFDcEQsYUFBTyxPQUFPLGFBQWEsU0FBUyxNQUFNLENBQUMsR0FBRyxFQUFFLENBQUM7QUFBQSxJQUNsRDtBQUVBLFFBQUssUUFBUSxXQUFXLE1BQU0sVUFBVSxHQUFJO0FBQzNDLGFBQU8sT0FBTyxhQUFhLENBQUMsQ0FBQyxNQUFNLENBQUMsQ0FBQztBQUFBLElBQ3RDO0FBQ0EsV0FBTztBQUFBLEVBQ1IsQ0FBQztBQUNGO0FBRUEsU0FBUyxlQUFlLE9BQXVCO0FBQzlDLE1BQUksU0FBUyxNQUFNLFFBQVEsT0FBTyxFQUFFO0FBRXBDLFNBQU8sT0FBTyxPQUFPLENBQUMsTUFBTSxRQUFRLE9BQU8sT0FBTyxDQUFDLE1BQU0sTUFBTTtBQUM5RCxhQUFTLE9BQU8sTUFBTSxDQUFDO0FBQUEsRUFDeEI7QUFDQSxTQUFPLE9BQU8sTUFBTSxFQUFFLE1BQU0sUUFBUSxPQUFPLE1BQU0sRUFBRSxNQUFNLE1BQU07QUFDOUQsYUFBUyxPQUFPLE1BQU0sR0FBRyxFQUFFO0FBQUEsRUFDNUI7QUFFQSxTQUFPO0FBQ1I7IiwKICAibmFtZXMiOiBbXQp9Cg==
