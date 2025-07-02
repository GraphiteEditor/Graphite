// vite.config.ts
import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";
import { svelte } from "file:///E:/Code/Graphite/frontend/node_modules/@sveltejs/vite-plugin-svelte/src/index.js";
import rollupPluginLicense from "file:///E:/Code/Graphite/frontend/node_modules/rollup-plugin-license/dist/index.js";
import { sveltePreprocess } from "file:///E:/Code/Graphite/frontend/node_modules/svelte-preprocess/dist/index.js";
import { defineConfig } from "file:///E:/Code/Graphite/frontend/node_modules/vite/dist/node/index.js";
import { DynamicPublicDirectory as viteMultipleAssets } from "file:///E:/Code/Graphite/frontend/node_modules/vite-multiple-assets/dist/index.mjs";
var __vite_injected_original_dirname = "E:\\Code\\Graphite\\frontend";
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
  "Unicode-3.0",
  "Unicode-DFS-2016",
  "Zlib"
];
var vite_config_default = defineConfig({
  plugins: [
    svelte({
      preprocess: [sveltePreprocess()],
      onwarn(warning, defaultHandler) {
        const suppressed = ["css-unused-selector", "vite-plugin-svelte-css-no-scopable-elements", "a11y-no-static-element-interactions", "a11y-no-noninteractive-element-interactions"];
        if (suppressed.includes(warning.code)) return;
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
  let foundLicensesIndex;
  let foundPackagesIndex;
  licenses.forEach((license, licenseIndex) => {
    license.packages.forEach((pkg, pkgIndex) => {
      if (pkg.name === "path-bool") {
        foundLicensesIndex = licenseIndex;
        foundPackagesIndex = pkgIndex;
      }
    });
  });
  if (foundLicensesIndex !== void 0 && foundPackagesIndex !== void 0) {
    const license = licenses[foundLicensesIndex];
    const pkg = license.packages[foundPackagesIndex];
    license.packages = license.packages.filter((pkg2) => pkg2.name !== "path-bool");
    const noticeText = fs.readFileSync(path.resolve(__vite_injected_original_dirname, "../libraries/path-bool/NOTICE"), "utf8");
    licenses.push({
      licenseName: license.licenseName,
      licenseText: license.licenseText,
      noticeText,
      packages: [pkg]
    });
  }
  jsLicenses.forEach((jsLicense) => {
    const name = jsLicense.name || "";
    const version = jsLicense.version || "";
    const author = jsLicense.author?.text() || "";
    const licenseName = jsLicense.license || "";
    const licenseText = trimBlankLines(jsLicense.licenseText || "");
    const noticeText = trimBlankLines(jsLicense.noticeText || "");
    let repository = jsLicense.repository || "";
    if (repository && typeof repository === "object") repository = repository.url;
    const repo = repository ? repository.replace(/^.*(github.com\/.*?\/.*?)(?:.git)/, "https://$1") : repository;
    const matchedLicense = licenses.find(
      (license) => license.licenseName === licenseName && trimBlankLines(license.licenseText || "") === licenseText && trimBlankLines(license.noticeText || "") === noticeText
    );
    const pkg = { name, version, author, repository: repo };
    if (matchedLicense) matchedLicense.packages.push(pkg);
    else licenses.push({ licenseName, licenseText, noticeText, packages: [pkg] });
  });
  licenses.forEach((license, index) => {
    if (license.noticeText) {
      licenses[index].licenseText += "\n\n";
      licenses[index].licenseText += " _______________________________________\n";
      licenses[index].licenseText += "\u2502                                       \u2502\n";
      licenses[index].licenseText += "\u2502 THE FOLLOWING NOTICE FILE IS INCLUDED \u2502\n";
      licenses[index].licenseText += "\u2502                                       \u2502\n";
      licenses[index].licenseText += " \u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\u203E\n\n";
      licenses[index].licenseText += `${license.noticeText}
`;
      licenses[index].noticeText = void 0;
    }
  });
  const licensesNormalizedWhitespace = licenses.map((license) => license.licenseText.replace(/[\n\s]+/g, " ").trim());
  licenses.forEach((currentLicense, currentLicenseIndex) => {
    licenses.slice(0, currentLicenseIndex).forEach((comparisonLicense, comparisonLicenseIndex) => {
      if (licensesNormalizedWhitespace[currentLicenseIndex] === licensesNormalizedWhitespace[comparisonLicenseIndex]) {
        currentLicense.packages.push(...comparisonLicense.packages);
        comparisonLicense.packages = [];
      }
    });
  });
  licenses = licenses.filter((license) => {
    license.packages = license.packages.filter(
      (packageInfo) => !(packageInfo.repository && packageInfo.repository.toLowerCase().includes("github.com/GraphiteEditor/Graphite".toLowerCase())) && !(packageInfo.author && packageInfo.author.toLowerCase().includes("contact@graphite.rs") && // Exclude a comma which indicates multiple authors, which we need to not filter out
      !packageInfo.author.toLowerCase().includes(","))
    );
    return license.packages.length > 0;
  });
  licenses.sort((a, b) => a.licenseText.localeCompare(b.licenseText));
  licenses.sort((a, b) => a.licenseName.localeCompare(b.licenseName));
  licenses.sort((a, b) => b.packages.length - a.packages.length);
  licenses.forEach((license) => {
    license.packages.sort((a, b) => a.name.localeCompare(b.name));
  });
  let formattedLicenseNotice = "";
  formattedLicenseNotice += "\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\n";
  formattedLicenseNotice += "\u2590\u2590                                                   \u2590\u2590\n";
  formattedLicenseNotice += "\u2590\u2590   GRAPHITE THIRD-PARTY SOFTWARE LICENSE NOTICES   \u2590\u2590\n";
  formattedLicenseNotice += "\u2590\u2590                                                   \u2590\u2590\n";
  formattedLicenseNotice += "\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\u2590\n";
  licenses.forEach((license) => {
    let packagesWithSameLicense = license.packages.map((packageInfo) => {
      const { name, version, author, repository } = packageInfo;
      return `${name} ${version}${author ? ` - ${author}` : ""}${repository ? ` - ${repository}` : ""}`;
    });
    const multi = packagesWithSameLicense.length !== 1;
    const saysLicense = license.licenseName.toLowerCase().includes("license");
    const header = `The package${multi ? "s" : ""} listed here ${multi ? "are" : "is"} licensed under the terms of the ${license.licenseName}${saysLicense ? "" : " license"} printed beneath`;
    const packagesLineLength = Math.max(header.length, ...packagesWithSameLicense.map((line) => line.length));
    packagesWithSameLicense = packagesWithSameLicense.map((line) => `\u2502 ${line}${" ".repeat(packagesLineLength - line.length)} \u2502`);
    formattedLicenseNotice += "\n";
    formattedLicenseNotice += ` ${"_".repeat(packagesLineLength + 2)}
`;
    formattedLicenseNotice += `\u2502 ${" ".repeat(packagesLineLength)} \u2502
`;
    formattedLicenseNotice += `\u2502 ${header}${" ".repeat(packagesLineLength - header.length)} \u2502
`;
    formattedLicenseNotice += `\u2502${"_".repeat(packagesLineLength + 2)}\u2502
`;
    formattedLicenseNotice += `${packagesWithSameLicense.join("\n")}
`;
    formattedLicenseNotice += ` ${"\u203E".repeat(packagesLineLength + 2)}
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
  if (!input) return input;
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
    const maybeEntity = Object.entries(htmlEntities).find(([key, _]) => key === entityCode);
    if (maybeEntity) return maybeEntity[1];
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
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcudHMiXSwKICAic291cmNlc0NvbnRlbnQiOiBbImNvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9kaXJuYW1lID0gXCJFOlxcXFxDb2RlXFxcXEdyYXBoaXRlXFxcXGZyb250ZW5kXCI7Y29uc3QgX192aXRlX2luamVjdGVkX29yaWdpbmFsX2ZpbGVuYW1lID0gXCJFOlxcXFxDb2RlXFxcXEdyYXBoaXRlXFxcXGZyb250ZW5kXFxcXHZpdGUuY29uZmlnLnRzXCI7Y29uc3QgX192aXRlX2luamVjdGVkX29yaWdpbmFsX2ltcG9ydF9tZXRhX3VybCA9IFwiZmlsZTovLy9FOi9Db2RlL0dyYXBoaXRlL2Zyb250ZW5kL3ZpdGUuY29uZmlnLnRzXCI7LyogZXNsaW50LWRpc2FibGUgbm8tY29uc29sZSAqL1xuXG5pbXBvcnQgeyBzcGF3blN5bmMgfSBmcm9tIFwiY2hpbGRfcHJvY2Vzc1wiO1xuXG5pbXBvcnQgZnMgZnJvbSBcImZzXCI7XG5pbXBvcnQgcGF0aCBmcm9tIFwicGF0aFwiO1xuXG5pbXBvcnQgeyBzdmVsdGUgfSBmcm9tIFwiQHN2ZWx0ZWpzL3ZpdGUtcGx1Z2luLXN2ZWx0ZVwiO1xuaW1wb3J0IHJvbGx1cFBsdWdpbkxpY2Vuc2UsIHsgdHlwZSBEZXBlbmRlbmN5IH0gZnJvbSBcInJvbGx1cC1wbHVnaW4tbGljZW5zZVwiO1xuaW1wb3J0IHsgc3ZlbHRlUHJlcHJvY2VzcyB9IGZyb20gXCJzdmVsdGUtcHJlcHJvY2Vzc1wiO1xuaW1wb3J0IHsgZGVmaW5lQ29uZmlnIH0gZnJvbSBcInZpdGVcIjtcbmltcG9ydCB7IER5bmFtaWNQdWJsaWNEaXJlY3RvcnkgYXMgdml0ZU11bHRpcGxlQXNzZXRzIH0gZnJvbSBcInZpdGUtbXVsdGlwbGUtYXNzZXRzXCI7XG5cbmNvbnN0IHByb2plY3RSb290RGlyID0gcGF0aC5yZXNvbHZlKF9fZGlybmFtZSk7XG5cbi8vIEtlZXAgdGhpcyBsaXN0IGluIHN5bmMgd2l0aCB0aG9zZSBpbiBgL2Fib3V0LnRvbWxgIGFuZCBgL2RlbnkudG9tbGAuXG5jb25zdCBBTExPV0VEX0xJQ0VOU0VTID0gW1xuXHRcIkFwYWNoZS0yLjAgV0lUSCBMTFZNLWV4Y2VwdGlvblwiLFxuXHRcIkFwYWNoZS0yLjBcIixcblx0XCJCU0QtMi1DbGF1c2VcIixcblx0XCJCU0QtMy1DbGF1c2VcIixcblx0XCJCU0wtMS4wXCIsXG5cdFwiQ0MwLTEuMFwiLFxuXHRcIklTQ1wiLFxuXHRcIk1JVC0wXCIsXG5cdFwiTUlUXCIsXG5cdFwiTVBMLTIuMFwiLFxuXHRcIk9wZW5TU0xcIixcblx0XCJVbmljb2RlLTMuMFwiLFxuXHRcIlVuaWNvZGUtREZTLTIwMTZcIixcblx0XCJabGliXCIsXG5dO1xuXG4vLyBodHRwczovL3ZpdGVqcy5kZXYvY29uZmlnL1xuZXhwb3J0IGRlZmF1bHQgZGVmaW5lQ29uZmlnKHtcblx0cGx1Z2luczogW1xuXHRcdHN2ZWx0ZSh7XG5cdFx0XHRwcmVwcm9jZXNzOiBbc3ZlbHRlUHJlcHJvY2VzcygpXSxcblx0XHRcdG9ud2Fybih3YXJuaW5nLCBkZWZhdWx0SGFuZGxlcikge1xuXHRcdFx0XHQvLyBOT1RJQ0U6IEtlZXAgdGhpcyBsaXN0IGluIHN5bmMgd2l0aCB0aGUgbGlzdCBpbiBgLnZzY29kZS9zZXR0aW5ncy5qc29uYFxuXHRcdFx0XHRjb25zdCBzdXBwcmVzc2VkID0gW1wiY3NzLXVudXNlZC1zZWxlY3RvclwiLCBcInZpdGUtcGx1Z2luLXN2ZWx0ZS1jc3Mtbm8tc2NvcGFibGUtZWxlbWVudHNcIiwgXCJhMTF5LW5vLXN0YXRpYy1lbGVtZW50LWludGVyYWN0aW9uc1wiLCBcImExMXktbm8tbm9uaW50ZXJhY3RpdmUtZWxlbWVudC1pbnRlcmFjdGlvbnNcIl07XG5cdFx0XHRcdGlmIChzdXBwcmVzc2VkLmluY2x1ZGVzKHdhcm5pbmcuY29kZSkpIHJldHVybjtcblxuXHRcdFx0XHRkZWZhdWx0SGFuZGxlcj8uKHdhcm5pbmcpO1xuXHRcdFx0fSxcblx0XHR9KSxcblx0XHR2aXRlTXVsdGlwbGVBc3NldHMoW1wiLi4vZGVtby1hcnR3b3JrXCJdKSxcblx0XSxcblx0cmVzb2x2ZToge1xuXHRcdGFsaWFzOiBbXG5cdFx0XHR7IGZpbmQ6IC9AZ3JhcGhpdGUtZnJvbnRlbmRcXC8oLipcXC5zdmcpLywgcmVwbGFjZW1lbnQ6IHBhdGgucmVzb2x2ZShwcm9qZWN0Um9vdERpciwgXCIkMT9yYXdcIikgfSxcblx0XHRcdHsgZmluZDogXCJAZ3JhcGhpdGUtZnJvbnRlbmRcIiwgcmVwbGFjZW1lbnQ6IHByb2plY3RSb290RGlyIH0sXG5cdFx0XHR7IGZpbmQ6IFwiQGdyYXBoaXRlLy4uL2Fzc2V0c1wiLCByZXBsYWNlbWVudDogcGF0aC5yZXNvbHZlKHByb2plY3RSb290RGlyLCBcImFzc2V0c1wiKSB9LFxuXHRcdFx0eyBmaW5kOiBcIkBncmFwaGl0ZS8uLi9wdWJsaWNcIiwgcmVwbGFjZW1lbnQ6IHBhdGgucmVzb2x2ZShwcm9qZWN0Um9vdERpciwgXCJwdWJsaWNcIikgfSxcblx0XHRcdHsgZmluZDogXCJAZ3JhcGhpdGVcIiwgcmVwbGFjZW1lbnQ6IHBhdGgucmVzb2x2ZShwcm9qZWN0Um9vdERpciwgXCJzcmNcIikgfSxcblx0XHRdLFxuXHR9LFxuXHRzZXJ2ZXI6IHtcblx0XHRwb3J0OiA4MDgwLFxuXHRcdGhvc3Q6IFwiMC4wLjAuMFwiLFxuXHR9LFxuXHRidWlsZDoge1xuXHRcdHJvbGx1cE9wdGlvbnM6IHtcblx0XHRcdHBsdWdpbnM6IFtcblx0XHRcdFx0cm9sbHVwUGx1Z2luTGljZW5zZSh7XG5cdFx0XHRcdFx0dGhpcmRQYXJ0eToge1xuXHRcdFx0XHRcdFx0YWxsb3c6IHtcblx0XHRcdFx0XHRcdFx0dGVzdDogYCgke0FMTE9XRURfTElDRU5TRVMuam9pbihcIiBPUiBcIil9KWAsXG5cdFx0XHRcdFx0XHRcdGZhaWxPblVubGljZW5zZWQ6IHRydWUsXG5cdFx0XHRcdFx0XHRcdGZhaWxPblZpb2xhdGlvbjogdHJ1ZSxcblx0XHRcdFx0XHRcdH0sXG5cdFx0XHRcdFx0XHRvdXRwdXQ6IHtcblx0XHRcdFx0XHRcdFx0ZmlsZTogcGF0aC5yZXNvbHZlKF9fZGlybmFtZSwgXCIuL2Rpc3QvdGhpcmQtcGFydHktbGljZW5zZXMudHh0XCIpLFxuXHRcdFx0XHRcdFx0XHR0ZW1wbGF0ZTogZm9ybWF0VGhpcmRQYXJ0eUxpY2Vuc2VzLFxuXHRcdFx0XHRcdFx0fSxcblx0XHRcdFx0XHR9LFxuXHRcdFx0XHR9KSxcblx0XHRcdF0sXG5cdFx0XHRvdXRwdXQ6IHtcblx0XHRcdFx0Ly8gSW5qZWN0IGAubWluYCBpbnRvIHRoZSBmaWxlbmFtZSBvZiBtaW5pZmllZCBDU1MgZmlsZXMgdG8gdGVsbCBDbG91ZGZsYXJlIG5vdCB0byBtaW5pZnkgaXQgYWdhaW4uXG5cdFx0XHRcdC8vIENsb3VkZmxhcmUncyBtaW5pZmllciBicmVha3MgdGhlIENTUyBkdWUgdG8gYSBidWcgd2hlcmUgaXQgcmVtb3ZlcyB3aGl0ZXNwYWNlIGFyb3VuZCBjYWxjKCkgcGx1cyBvcGVyYXRvcnMuXG5cdFx0XHRcdGFzc2V0RmlsZU5hbWVzOiAoaW5mbykgPT4gYGFzc2V0cy9bbmFtZV0tW2hhc2hdJHtpbmZvLm5hbWU/LmVuZHNXaXRoKFwiLmNzc1wiKSA/IFwiLm1pblwiIDogXCJcIn1bZXh0bmFtZV1gLFxuXHRcdFx0fSxcblx0XHR9LFxuXHR9LFxufSk7XG5cbnR5cGUgTGljZW5zZUluZm8gPSB7XG5cdGxpY2Vuc2VOYW1lOiBzdHJpbmc7XG5cdGxpY2Vuc2VUZXh0OiBzdHJpbmc7XG5cdG5vdGljZVRleHQ/OiBzdHJpbmc7XG5cdHBhY2thZ2VzOiBQYWNrYWdlSW5mb1tdO1xufTtcblxudHlwZSBQYWNrYWdlSW5mbyA9IHtcblx0bmFtZTogc3RyaW5nO1xuXHR2ZXJzaW9uOiBzdHJpbmc7XG5cdGF1dGhvcjogc3RyaW5nO1xuXHRyZXBvc2l0b3J5OiBzdHJpbmc7XG59O1xuXG5mdW5jdGlvbiBmb3JtYXRUaGlyZFBhcnR5TGljZW5zZXMoanNMaWNlbnNlczogRGVwZW5kZW5jeVtdKTogc3RyaW5nIHtcblx0Ly8gR2VuZXJhdGUgdGhlIFJ1c3QgbGljZW5zZSBpbmZvcm1hdGlvbi5cblx0bGV0IGxpY2Vuc2VzID0gZ2VuZXJhdGVSdXN0TGljZW5zZXMoKSB8fCBbXTtcblxuXHQvLyBFbnN1cmUgd2UgaGF2ZSBsaWNlbnNlIGluZm9ybWF0aW9uIHRvIHdvcmsgd2l0aCBiZWZvcmUgcHJvY2VlZGluZy5cblx0aWYgKGxpY2Vuc2VzLmxlbmd0aCA9PT0gMCkge1xuXHRcdC8vIFRoaXMgaXMgcHJvYmFibHkgY2F1c2VkIGJ5IGBjYXJnbyBhYm91dGAgbm90IGJlaW5nIGluc3RhbGxlZC5cblx0XHRjb25zb2xlLmVycm9yKFwiQ291bGQgbm90IHJ1biBgY2FyZ28gYWJvdXRgLCB3aGljaCBpcyByZXF1aXJlZCB0byBnZW5lcmF0ZSBsaWNlbnNlIGluZm9ybWF0aW9uLlwiKTtcblx0XHRjb25zb2xlLmVycm9yKFwiVG8gaW5zdGFsbCBjYXJnby1hYm91dCBvbiB5b3VyIHN5c3RlbSwgeW91IGNhbiBydW4gYGNhcmdvIGluc3RhbGwgY2FyZ28tYWJvdXRgLlwiKTtcblx0XHRjb25zb2xlLmVycm9yKFwiTGljZW5zZSBpbmZvcm1hdGlvbiBpcyByZXF1aXJlZCBpbiBwcm9kdWN0aW9uIGJ1aWxkcy4gQWJvcnRpbmcuXCIpO1xuXG5cdFx0cHJvY2Vzcy5leGl0KDEpO1xuXHR9XG5cdGlmIChqc0xpY2Vuc2VzLmxlbmd0aCA9PT0gMCkge1xuXHRcdGNvbnNvbGUuZXJyb3IoXCJObyBKYXZhU2NyaXB0IHBhY2thZ2UgbGljZW5zZXMgd2VyZSBmb3VuZCBieSBgcm9sbHVwLXBsdWdpbi1saWNlbnNlYC4gUGxlYXNlIGludmVzdGlnYXRlLlwiKTtcblx0XHRjb25zb2xlLmVycm9yKFwiTGljZW5zZSBpbmZvcm1hdGlvbiBpcyByZXF1aXJlZCBpbiBwcm9kdWN0aW9uIGJ1aWxkcy4gQWJvcnRpbmcuXCIpO1xuXG5cdFx0cHJvY2Vzcy5leGl0KDEpO1xuXHR9XG5cblx0Ly8gRmluZCB0aGVuIGR1cGxpY2F0ZSB0aGlzIGxpY2Vuc2UgaWYgb25lIG9mIGl0cyBwYWNrYWdlcyBpcyBgcGF0aC1ib29sYCwgYWRkaW5nIGl0cyBub3RpY2UgdGV4dC5cblx0bGV0IGZvdW5kTGljZW5zZXNJbmRleDtcblx0bGV0IGZvdW5kUGFja2FnZXNJbmRleDtcblx0bGljZW5zZXMuZm9yRWFjaCgobGljZW5zZSwgbGljZW5zZUluZGV4KSA9PiB7XG5cdFx0bGljZW5zZS5wYWNrYWdlcy5mb3JFYWNoKChwa2csIHBrZ0luZGV4KSA9PiB7XG5cdFx0XHRpZiAocGtnLm5hbWUgPT09IFwicGF0aC1ib29sXCIpIHtcblx0XHRcdFx0Zm91bmRMaWNlbnNlc0luZGV4ID0gbGljZW5zZUluZGV4O1xuXHRcdFx0XHRmb3VuZFBhY2thZ2VzSW5kZXggPSBwa2dJbmRleDtcblx0XHRcdH1cblx0XHR9KTtcblx0fSk7XG5cdGlmIChmb3VuZExpY2Vuc2VzSW5kZXggIT09IHVuZGVmaW5lZCAmJiBmb3VuZFBhY2thZ2VzSW5kZXggIT09IHVuZGVmaW5lZCkge1xuXHRcdGNvbnN0IGxpY2Vuc2UgPSBsaWNlbnNlc1tmb3VuZExpY2Vuc2VzSW5kZXhdO1xuXHRcdGNvbnN0IHBrZyA9IGxpY2Vuc2UucGFja2FnZXNbZm91bmRQYWNrYWdlc0luZGV4XTtcblxuXHRcdGxpY2Vuc2UucGFja2FnZXMgPSBsaWNlbnNlLnBhY2thZ2VzLmZpbHRlcigocGtnKSA9PiBwa2cubmFtZSAhPT0gXCJwYXRoLWJvb2xcIik7XG5cdFx0Y29uc3Qgbm90aWNlVGV4dCA9IGZzLnJlYWRGaWxlU3luYyhwYXRoLnJlc29sdmUoX19kaXJuYW1lLCBcIi4uL2xpYnJhcmllcy9wYXRoLWJvb2wvTk9USUNFXCIpLCBcInV0ZjhcIik7XG5cblx0XHRsaWNlbnNlcy5wdXNoKHtcblx0XHRcdGxpY2Vuc2VOYW1lOiBsaWNlbnNlLmxpY2Vuc2VOYW1lLFxuXHRcdFx0bGljZW5zZVRleHQ6IGxpY2Vuc2UubGljZW5zZVRleHQsXG5cdFx0XHRub3RpY2VUZXh0LFxuXHRcdFx0cGFja2FnZXM6IFtwa2ddLFxuXHRcdH0pO1xuXHR9XG5cblx0Ly8gQXVnbWVudCB0aGUgaW1wb3J0ZWQgUnVzdCBsaWNlbnNlIGxpc3Qgd2l0aCB0aGUgcHJvdmlkZWQgSlMgbGljZW5zZSBsaXN0LlxuXHRqc0xpY2Vuc2VzLmZvckVhY2goKGpzTGljZW5zZSkgPT4ge1xuXHRcdGNvbnN0IG5hbWUgPSBqc0xpY2Vuc2UubmFtZSB8fCBcIlwiO1xuXHRcdGNvbnN0IHZlcnNpb24gPSBqc0xpY2Vuc2UudmVyc2lvbiB8fCBcIlwiO1xuXHRcdGNvbnN0IGF1dGhvciA9IGpzTGljZW5zZS5hdXRob3I/LnRleHQoKSB8fCBcIlwiO1xuXHRcdGNvbnN0IGxpY2Vuc2VOYW1lID0ganNMaWNlbnNlLmxpY2Vuc2UgfHwgXCJcIjtcblx0XHRjb25zdCBsaWNlbnNlVGV4dCA9IHRyaW1CbGFua0xpbmVzKGpzTGljZW5zZS5saWNlbnNlVGV4dCB8fCBcIlwiKTtcblx0XHRjb25zdCBub3RpY2VUZXh0ID0gdHJpbUJsYW5rTGluZXMoanNMaWNlbnNlLm5vdGljZVRleHQgfHwgXCJcIik7XG5cblx0XHRsZXQgcmVwb3NpdG9yeSA9IGpzTGljZW5zZS5yZXBvc2l0b3J5IHx8IFwiXCI7XG5cdFx0aWYgKHJlcG9zaXRvcnkgJiYgdHlwZW9mIHJlcG9zaXRvcnkgPT09IFwib2JqZWN0XCIpIHJlcG9zaXRvcnkgPSByZXBvc2l0b3J5LnVybDtcblx0XHQvLyBSZW1vdmUgdGhlIGBnaXQrYCBvciBgZ2l0Oi8vYCBwcmVmaXggYW5kIGAuZ2l0YCBzdWZmaXguXG5cdFx0Y29uc3QgcmVwbyA9IHJlcG9zaXRvcnkgPyByZXBvc2l0b3J5LnJlcGxhY2UoL14uKihnaXRodWIuY29tXFwvLio/XFwvLio/KSg/Oi5naXQpLywgXCJodHRwczovLyQxXCIpIDogcmVwb3NpdG9yeTtcblxuXHRcdGNvbnN0IG1hdGNoZWRMaWNlbnNlID0gbGljZW5zZXMuZmluZChcblx0XHRcdChsaWNlbnNlKSA9PiBsaWNlbnNlLmxpY2Vuc2VOYW1lID09PSBsaWNlbnNlTmFtZSAmJiB0cmltQmxhbmtMaW5lcyhsaWNlbnNlLmxpY2Vuc2VUZXh0IHx8IFwiXCIpID09PSBsaWNlbnNlVGV4dCAmJiB0cmltQmxhbmtMaW5lcyhsaWNlbnNlLm5vdGljZVRleHQgfHwgXCJcIikgPT09IG5vdGljZVRleHQsXG5cdFx0KTtcblxuXHRcdGNvbnN0IHBrZzogUGFja2FnZUluZm8gPSB7IG5hbWUsIHZlcnNpb24sIGF1dGhvciwgcmVwb3NpdG9yeTogcmVwbyB9O1xuXHRcdGlmIChtYXRjaGVkTGljZW5zZSkgbWF0Y2hlZExpY2Vuc2UucGFja2FnZXMucHVzaChwa2cpO1xuXHRcdGVsc2UgbGljZW5zZXMucHVzaCh7IGxpY2Vuc2VOYW1lLCBsaWNlbnNlVGV4dCwgbm90aWNlVGV4dCwgcGFja2FnZXM6IFtwa2ddIH0pO1xuXHR9KTtcblxuXHQvLyBDb21iaW5lIGFueSBsaWNlbnNlIG5vdGljZXMgaW50byB0aGUgbGljZW5zZSB0ZXh0LlxuXHRsaWNlbnNlcy5mb3JFYWNoKChsaWNlbnNlLCBpbmRleCkgPT4ge1xuXHRcdGlmIChsaWNlbnNlLm5vdGljZVRleHQpIHtcblx0XHRcdGxpY2Vuc2VzW2luZGV4XS5saWNlbnNlVGV4dCArPSBcIlxcblxcblwiO1xuXHRcdFx0bGljZW5zZXNbaW5kZXhdLmxpY2Vuc2VUZXh0ICs9IFwiIF9fX19fX19fX19fX19fX19fX19fX19fX19fX19fX19fX19fX19fX1xcblwiO1xuXHRcdFx0bGljZW5zZXNbaW5kZXhdLmxpY2Vuc2VUZXh0ICs9IFwiXHUyNTAyICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgXHUyNTAyXFxuXCI7XG5cdFx0XHRsaWNlbnNlc1tpbmRleF0ubGljZW5zZVRleHQgKz0gXCJcdTI1MDIgVEhFIEZPTExPV0lORyBOT1RJQ0UgRklMRSBJUyBJTkNMVURFRCBcdTI1MDJcXG5cIjtcblx0XHRcdGxpY2Vuc2VzW2luZGV4XS5saWNlbnNlVGV4dCArPSBcIlx1MjUwMiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIFx1MjUwMlxcblwiO1xuXHRcdFx0bGljZW5zZXNbaW5kZXhdLmxpY2Vuc2VUZXh0ICs9IFwiIFx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVx1MjAzRVxcblxcblwiO1xuXHRcdFx0bGljZW5zZXNbaW5kZXhdLmxpY2Vuc2VUZXh0ICs9IGAke2xpY2Vuc2Uubm90aWNlVGV4dH1cXG5gO1xuXHRcdFx0bGljZW5zZXNbaW5kZXhdLm5vdGljZVRleHQgPSB1bmRlZmluZWQ7XG5cdFx0fVxuXHR9KTtcblxuXHQvLyBEZS1kdXBsaWNhdGUgYW55IGxpY2Vuc2VzIHdpdGggdGhlIHNhbWUgdGV4dCBieSBtZXJnaW5nIHRoZWlyIGxpc3RzIG9mIHBhY2thZ2VzLlxuXHRjb25zdCBsaWNlbnNlc05vcm1hbGl6ZWRXaGl0ZXNwYWNlID0gbGljZW5zZXMubWFwKChsaWNlbnNlKSA9PiBsaWNlbnNlLmxpY2Vuc2VUZXh0LnJlcGxhY2UoL1tcXG5cXHNdKy9nLCBcIiBcIikudHJpbSgpKTtcblx0bGljZW5zZXMuZm9yRWFjaCgoY3VycmVudExpY2Vuc2UsIGN1cnJlbnRMaWNlbnNlSW5kZXgpID0+IHtcblx0XHRsaWNlbnNlcy5zbGljZSgwLCBjdXJyZW50TGljZW5zZUluZGV4KS5mb3JFYWNoKChjb21wYXJpc29uTGljZW5zZSwgY29tcGFyaXNvbkxpY2Vuc2VJbmRleCkgPT4ge1xuXHRcdFx0aWYgKGxpY2Vuc2VzTm9ybWFsaXplZFdoaXRlc3BhY2VbY3VycmVudExpY2Vuc2VJbmRleF0gPT09IGxpY2Vuc2VzTm9ybWFsaXplZFdoaXRlc3BhY2VbY29tcGFyaXNvbkxpY2Vuc2VJbmRleF0pIHtcblx0XHRcdFx0Y3VycmVudExpY2Vuc2UucGFja2FnZXMucHVzaCguLi5jb21wYXJpc29uTGljZW5zZS5wYWNrYWdlcyk7XG5cdFx0XHRcdGNvbXBhcmlzb25MaWNlbnNlLnBhY2thZ2VzID0gW107XG5cdFx0XHRcdC8vIEFmdGVyIGVtcHR5aW5nIHRoZSBwYWNrYWdlcywgdGhlIHJlZHVuZGFudCBsaWNlbnNlIHdpdGggbm8gcGFja2FnZXMgd2lsbCBiZSByZW1vdmVkIGluIHRoZSBuZXh0IHN0ZXAncyBgZmlsdGVyKClgLlxuXHRcdFx0fVxuXHRcdH0pO1xuXHR9KTtcblxuXHQvLyBGaWx0ZXIgb3V0IGZpcnN0LXBhcnR5IGludGVybmFsIEdyYXBoaXRlIGNyYXRlcy5cblx0bGljZW5zZXMgPSBsaWNlbnNlcy5maWx0ZXIoKGxpY2Vuc2UpID0+IHtcblx0XHRsaWNlbnNlLnBhY2thZ2VzID0gbGljZW5zZS5wYWNrYWdlcy5maWx0ZXIoXG5cdFx0XHQocGFja2FnZUluZm8pID0+XG5cdFx0XHRcdCEocGFja2FnZUluZm8ucmVwb3NpdG9yeSAmJiBwYWNrYWdlSW5mby5yZXBvc2l0b3J5LnRvTG93ZXJDYXNlKCkuaW5jbHVkZXMoXCJnaXRodWIuY29tL0dyYXBoaXRlRWRpdG9yL0dyYXBoaXRlXCIudG9Mb3dlckNhc2UoKSkpICYmXG5cdFx0XHRcdCEoXG5cdFx0XHRcdFx0cGFja2FnZUluZm8uYXV0aG9yICYmXG5cdFx0XHRcdFx0cGFja2FnZUluZm8uYXV0aG9yLnRvTG93ZXJDYXNlKCkuaW5jbHVkZXMoXCJjb250YWN0QGdyYXBoaXRlLnJzXCIpICYmXG5cdFx0XHRcdFx0Ly8gRXhjbHVkZSBhIGNvbW1hIHdoaWNoIGluZGljYXRlcyBtdWx0aXBsZSBhdXRob3JzLCB3aGljaCB3ZSBuZWVkIHRvIG5vdCBmaWx0ZXIgb3V0XG5cdFx0XHRcdFx0IXBhY2thZ2VJbmZvLmF1dGhvci50b0xvd2VyQ2FzZSgpLmluY2x1ZGVzKFwiLFwiKVxuXHRcdFx0XHQpLFxuXHRcdCk7XG5cdFx0cmV0dXJuIGxpY2Vuc2UucGFja2FnZXMubGVuZ3RoID4gMDtcblx0fSk7XG5cblx0Ly8gU29ydCB0aGUgbGljZW5zZXMgYnkgdGhlIG51bWJlciBvZiBwYWNrYWdlcyB1c2luZyB0aGUgc2FtZSBsaWNlbnNlLCBhbmQgdGhlbiBhbHBoYWJldGljYWxseSBieSBsaWNlbnNlIG5hbWUuXG5cdGxpY2Vuc2VzLnNvcnQoKGEsIGIpID0+IGEubGljZW5zZVRleHQubG9jYWxlQ29tcGFyZShiLmxpY2Vuc2VUZXh0KSk7XG5cdGxpY2Vuc2VzLnNvcnQoKGEsIGIpID0+IGEubGljZW5zZU5hbWUubG9jYWxlQ29tcGFyZShiLmxpY2Vuc2VOYW1lKSk7XG5cdGxpY2Vuc2VzLnNvcnQoKGEsIGIpID0+IGIucGFja2FnZXMubGVuZ3RoIC0gYS5wYWNrYWdlcy5sZW5ndGgpO1xuXHQvLyBTb3J0IHRoZSBpbmRpdmlkdWFsIHBhY2thZ2VzIHVzaW5nIGVhY2ggbGljZW5zZSBhbHBoYWJldGljYWxseS5cblx0bGljZW5zZXMuZm9yRWFjaCgobGljZW5zZSkgPT4ge1xuXHRcdGxpY2Vuc2UucGFja2FnZXMuc29ydCgoYSwgYikgPT4gYS5uYW1lLmxvY2FsZUNvbXBhcmUoYi5uYW1lKSk7XG5cdH0pO1xuXG5cdC8vIFByZXBhcmUgYSBoZWFkZXIgZm9yIHRoZSBsaWNlbnNlIG5vdGljZS5cblx0bGV0IGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgPSBcIlwiO1xuXHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IFwiXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXFxuXCI7XG5cdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gXCJcdTI1OTBcdTI1OTAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICBcdTI1OTBcdTI1OTBcXG5cIjtcblx0Zm9ybWF0dGVkTGljZW5zZU5vdGljZSArPSBcIlx1MjU5MFx1MjU5MCAgIEdSQVBISVRFIFRISVJELVBBUlRZIFNPRlRXQVJFIExJQ0VOU0UgTk9USUNFUyAgIFx1MjU5MFx1MjU5MFxcblwiO1xuXHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IFwiXHUyNTkwXHUyNTkwICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgXHUyNTkwXHUyNTkwXFxuXCI7XG5cdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gXCJcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcdTI1OTBcXG5cIjtcblxuXHQvLyBBcHBlbmQgYSBibG9jayBmb3IgZWFjaCBsaWNlbnNlIHNoYXJlZCBieSBtdWx0aXBsZSBwYWNrYWdlcyB3aXRoIGlkZW50aWNhbCBsaWNlbnNlIHRleHQuXG5cdGxpY2Vuc2VzLmZvckVhY2goKGxpY2Vuc2UpID0+IHtcblx0XHRsZXQgcGFja2FnZXNXaXRoU2FtZUxpY2Vuc2UgPSBsaWNlbnNlLnBhY2thZ2VzLm1hcCgocGFja2FnZUluZm8pID0+IHtcblx0XHRcdGNvbnN0IHsgbmFtZSwgdmVyc2lvbiwgYXV0aG9yLCByZXBvc2l0b3J5IH0gPSBwYWNrYWdlSW5mbztcblx0XHRcdHJldHVybiBgJHtuYW1lfSAke3ZlcnNpb259JHthdXRob3IgPyBgIC0gJHthdXRob3J9YCA6IFwiXCJ9JHtyZXBvc2l0b3J5ID8gYCAtICR7cmVwb3NpdG9yeX1gIDogXCJcIn1gO1xuXHRcdH0pO1xuXHRcdGNvbnN0IG11bHRpID0gcGFja2FnZXNXaXRoU2FtZUxpY2Vuc2UubGVuZ3RoICE9PSAxO1xuXHRcdGNvbnN0IHNheXNMaWNlbnNlID0gbGljZW5zZS5saWNlbnNlTmFtZS50b0xvd2VyQ2FzZSgpLmluY2x1ZGVzKFwibGljZW5zZVwiKTtcblx0XHRjb25zdCBoZWFkZXIgPSBgVGhlIHBhY2thZ2Uke211bHRpID8gXCJzXCIgOiBcIlwifSBsaXN0ZWQgaGVyZSAke211bHRpID8gXCJhcmVcIiA6IFwiaXNcIn0gbGljZW5zZWQgdW5kZXIgdGhlIHRlcm1zIG9mIHRoZSAke2xpY2Vuc2UubGljZW5zZU5hbWV9JHtzYXlzTGljZW5zZSA/IFwiXCIgOiBcIiBsaWNlbnNlXCJ9IHByaW50ZWQgYmVuZWF0aGA7XG5cdFx0Y29uc3QgcGFja2FnZXNMaW5lTGVuZ3RoID0gTWF0aC5tYXgoaGVhZGVyLmxlbmd0aCwgLi4ucGFja2FnZXNXaXRoU2FtZUxpY2Vuc2UubWFwKChsaW5lKSA9PiBsaW5lLmxlbmd0aCkpO1xuXHRcdHBhY2thZ2VzV2l0aFNhbWVMaWNlbnNlID0gcGFja2FnZXNXaXRoU2FtZUxpY2Vuc2UubWFwKChsaW5lKSA9PiBgXHUyNTAyICR7bGluZX0ke1wiIFwiLnJlcGVhdChwYWNrYWdlc0xpbmVMZW5ndGggLSBsaW5lLmxlbmd0aCl9IFx1MjUwMmApO1xuXG5cdFx0Zm9ybWF0dGVkTGljZW5zZU5vdGljZSArPSBcIlxcblwiO1xuXHRcdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gYCAke1wiX1wiLnJlcGVhdChwYWNrYWdlc0xpbmVMZW5ndGggKyAyKX1cXG5gO1xuXHRcdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gYFx1MjUwMiAke1wiIFwiLnJlcGVhdChwYWNrYWdlc0xpbmVMZW5ndGgpfSBcdTI1MDJcXG5gO1xuXHRcdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gYFx1MjUwMiAke2hlYWRlcn0ke1wiIFwiLnJlcGVhdChwYWNrYWdlc0xpbmVMZW5ndGggLSBoZWFkZXIubGVuZ3RoKX0gXHUyNTAyXFxuYDtcblx0XHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IGBcdTI1MDIke1wiX1wiLnJlcGVhdChwYWNrYWdlc0xpbmVMZW5ndGggKyAyKX1cdTI1MDJcXG5gO1xuXHRcdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gYCR7cGFja2FnZXNXaXRoU2FtZUxpY2Vuc2Uuam9pbihcIlxcblwiKX1cXG5gO1xuXHRcdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gYCAke1wiXHUyMDNFXCIucmVwZWF0KHBhY2thZ2VzTGluZUxlbmd0aCArIDIpfVxcbmA7XG5cdFx0Zm9ybWF0dGVkTGljZW5zZU5vdGljZSArPSBgJHtsaWNlbnNlLmxpY2Vuc2VUZXh0fVxcbmA7XG5cdH0pO1xuXHRyZXR1cm4gZm9ybWF0dGVkTGljZW5zZU5vdGljZTtcbn1cblxuZnVuY3Rpb24gZ2VuZXJhdGVSdXN0TGljZW5zZXMoKTogTGljZW5zZUluZm9bXSB8IHVuZGVmaW5lZCB7XG5cdC8vIExvZyB0aGUgc3RhcnRpbmcgc3RhdHVzIHRvIHRoZSBidWlsZCBvdXRwdXQuXG5cdGNvbnNvbGUuaW5mbyhcIlxcblxcbkdlbmVyYXRpbmcgbGljZW5zZSBpbmZvcm1hdGlvbiBmb3IgUnVzdCBjb2RlXFxuXCIpO1xuXG5cdHRyeSB7XG5cdFx0Ly8gQ2FsbCBgY2FyZ28gYWJvdXRgIGluIHRoZSB0ZXJtaW5hbCB0byBnZW5lcmF0ZSB0aGUgbGljZW5zZSBpbmZvcm1hdGlvbiBmb3IgUnVzdCBjcmF0ZXMuXG5cdFx0Ly8gVGhlIGBhYm91dC5oYnNgIGZpbGUgaXMgd3JpdHRlbiBzbyBpdCBnZW5lcmF0ZXMgYSB2YWxpZCBKYXZhU2NyaXB0IGFycmF5IGV4cHJlc3Npb24gd2hpY2ggd2UgZXZhbHVhdGUgYmVsb3cuXG5cdFx0Y29uc3QgeyBzdGRvdXQsIHN0ZGVyciwgc3RhdHVzIH0gPSBzcGF3blN5bmMoXCJjYXJnb1wiLCBbXCJhYm91dFwiLCBcImdlbmVyYXRlXCIsIFwiYWJvdXQuaGJzXCJdLCB7XG5cdFx0XHRjd2Q6IHBhdGguam9pbihfX2Rpcm5hbWUsIFwiLi5cIiksXG5cdFx0XHRlbmNvZGluZzogXCJ1dGY4XCIsXG5cdFx0XHR0aW1lb3V0OiA2MDAwMCwgLy8gT25lIG1pbnV0ZVxuXHRcdFx0c2hlbGw6IHRydWUsXG5cdFx0XHR3aW5kb3dzSGlkZTogdHJ1ZSwgLy8gSGlkZSB0aGUgdGVybWluYWwgb24gV2luZG93c1xuXHRcdH0pO1xuXG5cdFx0Ly8gSWYgdGhlIGNvbW1hbmQgZmFpbGVkLCBwcmludCB0aGUgZXJyb3IgbWVzc2FnZSBhbmQgZXhpdCBlYXJseS5cblx0XHRpZiAoc3RhdHVzICE9PSAwKSB7XG5cdFx0XHQvLyBDYXJnbyByZXR1cm5zIDEwMSB3aGVuIHRoZSBzdWJjb21tYW5kIChgYWJvdXRgKSB3YXNuJ3QgZm91bmQsIHNvIHdlIHNraXAgcHJpbnRpbmcgdGhlIGJlbG93IGVycm9yIG1lc3NhZ2UgaW4gdGhhdCBjYXNlLlxuXHRcdFx0aWYgKHN0YXR1cyAhPT0gMTAxKSB7XG5cdFx0XHRcdGNvbnNvbGUuZXJyb3IoXCJjYXJnby1hYm91dCBmYWlsZWRcIiwgc3RhdHVzLCBzdGRlcnIpO1xuXHRcdFx0fVxuXHRcdFx0cmV0dXJuIHVuZGVmaW5lZDtcblx0XHR9XG5cblx0XHQvLyBNYWtlIHN1cmUgdGhlIG91dHB1dCBzdGFydHMgd2l0aCB0aGlzIGV4cGVjdGVkIGxhYmVsLCB3aGljaCBsZXRzIHVzIGtub3cgdGhlIGZpbGUgZ2VuZXJhdGVkIHdpdGggZXhwZWN0ZWQgb3V0cHV0LlxuXHRcdC8vIFdlIGRvbid0IHdhbnQgdG8gZXZhbCBhbiBlcnJvciBtZXNzYWdlIG9yIHNvbWV0aGluZyBlbHNlLCBzbyB3ZSBmYWlsIGVhcmx5IGlmIHRoYXQgaGFwcGVucy5cblx0XHRpZiAoIXN0ZG91dC50cmltKCkuc3RhcnRzV2l0aChcIkdFTkVSQVRFRF9CWV9DQVJHT19BQk9VVDpcIikpIHtcblx0XHRcdGNvbnNvbGUuZXJyb3IoXCJVbmV4cGVjdGVkIG91dHB1dCBmcm9tIGNhcmdvLWFib3V0XCIsIHN0ZG91dCk7XG5cdFx0XHRyZXR1cm4gdW5kZWZpbmVkO1xuXHRcdH1cblxuXHRcdC8vIENvbnZlcnQgdGhlIGFycmF5IEpTIHN5bnRheCBzdHJpbmcgaW50byBhbiBhY3R1YWwgSlMgYXJyYXkgaW4gbWVtb3J5LlxuXHRcdC8vIFNlY3VyaXR5LXdpc2UsIGV2YWwoKSBpc24ndCBhbnkgd29yc2UgdGhhbiByZXF1aXJlKCksIGJ1dCBpdCdzIGFibGUgdG8gd29yayB3aXRob3V0IGEgdGVtcG9yYXJ5IGZpbGUuXG5cdFx0Ly8gV2UgY2FsbCBldmFsIGluZGlyZWN0bHkgdG8gYXZvaWQgYSB3YXJuaW5nIGFzIGV4cGxhaW5lZCBoZXJlOiA8aHR0cHM6Ly9lc2J1aWxkLmdpdGh1Yi5pby9jb250ZW50LXR5cGVzLyNkaXJlY3QtZXZhbD4uXG5cdFx0Y29uc3QgaW5kaXJlY3RFdmFsID0gZXZhbDtcblx0XHRjb25zdCBsaWNlbnNlc0FycmF5ID0gaW5kaXJlY3RFdmFsKHN0ZG91dCkgYXMgTGljZW5zZUluZm9bXTtcblxuXHRcdC8vIFJlbW92ZSB0aGUgSFRNTCBjaGFyYWN0ZXIgZW5jb2RpbmcgY2F1c2VkIGJ5IEhhbmRsZWJhcnMuXG5cdFx0Y29uc3QgcnVzdExpY2Vuc2VzID0gKGxpY2Vuc2VzQXJyYXkgfHwgW10pLm1hcChcblx0XHRcdChydXN0TGljZW5zZSk6IExpY2Vuc2VJbmZvID0+ICh7XG5cdFx0XHRcdGxpY2Vuc2VOYW1lOiBodG1sRGVjb2RlKHJ1c3RMaWNlbnNlLmxpY2Vuc2VOYW1lKSxcblx0XHRcdFx0bGljZW5zZVRleHQ6IHRyaW1CbGFua0xpbmVzKGh0bWxEZWNvZGUocnVzdExpY2Vuc2UubGljZW5zZVRleHQpKSxcblx0XHRcdFx0cGFja2FnZXM6IHJ1c3RMaWNlbnNlLnBhY2thZ2VzLm1hcChcblx0XHRcdFx0XHQocGFja2FnZUluZm8pOiBQYWNrYWdlSW5mbyA9PiAoe1xuXHRcdFx0XHRcdFx0bmFtZTogaHRtbERlY29kZShwYWNrYWdlSW5mby5uYW1lKSxcblx0XHRcdFx0XHRcdHZlcnNpb246IGh0bWxEZWNvZGUocGFja2FnZUluZm8udmVyc2lvbiksXG5cdFx0XHRcdFx0XHRhdXRob3I6IGh0bWxEZWNvZGUocGFja2FnZUluZm8uYXV0aG9yKVxuXHRcdFx0XHRcdFx0XHQucmVwbGFjZSgvXFxbKC4qKSwgXFxdLywgXCIkMVwiKVxuXHRcdFx0XHRcdFx0XHQucmVwbGFjZShcIltdXCIsIFwiXCIpLFxuXHRcdFx0XHRcdFx0cmVwb3NpdG9yeTogaHRtbERlY29kZShwYWNrYWdlSW5mby5yZXBvc2l0b3J5KSxcblx0XHRcdFx0XHR9KSxcblx0XHRcdFx0KSxcblx0XHRcdH0pLFxuXHRcdCk7XG5cblx0XHRyZXR1cm4gcnVzdExpY2Vuc2VzO1xuXHR9IGNhdGNoIChfKSB7XG5cdFx0cmV0dXJuIHVuZGVmaW5lZDtcblx0fVxufVxuXG5mdW5jdGlvbiBodG1sRGVjb2RlKGlucHV0OiBzdHJpbmcpOiBzdHJpbmcge1xuXHRpZiAoIWlucHV0KSByZXR1cm4gaW5wdXQ7XG5cblx0Y29uc3QgaHRtbEVudGl0aWVzID0ge1xuXHRcdG5ic3A6IFwiIFwiLFxuXHRcdGNvcHk6IFwiXHUwMEE5XCIsXG5cdFx0cmVnOiBcIlx1MDBBRVwiLFxuXHRcdGx0OiBcIjxcIixcblx0XHRndDogXCI+XCIsXG5cdFx0YW1wOiBcIiZcIixcblx0XHRhcG9zOiBcIidcIixcblx0XHRxdW90OiBgXCJgLFxuXHR9O1xuXG5cdHJldHVybiBpbnB1dC5yZXBsYWNlKC8mKFteO10rKTsvZywgKGVudGl0eTogc3RyaW5nLCBlbnRpdHlDb2RlOiBzdHJpbmcpID0+IHtcblx0XHRjb25zdCBtYXliZUVudGl0eSA9IE9iamVjdC5lbnRyaWVzKGh0bWxFbnRpdGllcykuZmluZCgoW2tleSwgX10pID0+IGtleSA9PT0gZW50aXR5Q29kZSk7XG5cdFx0aWYgKG1heWJlRW50aXR5KSByZXR1cm4gbWF5YmVFbnRpdHlbMV07XG5cblx0XHRsZXQgbWF0Y2g7XG5cdFx0Ly8gZXNsaW50LWRpc2FibGUtbmV4dC1saW5lIG5vLWNvbmQtYXNzaWduXG5cdFx0aWYgKChtYXRjaCA9IGVudGl0eUNvZGUubWF0Y2goL14jeChbXFxkYS1mQS1GXSspJC8pKSkge1xuXHRcdFx0cmV0dXJuIFN0cmluZy5mcm9tQ2hhckNvZGUocGFyc2VJbnQobWF0Y2hbMV0sIDE2KSk7XG5cdFx0fVxuXHRcdC8vIGVzbGludC1kaXNhYmxlLW5leHQtbGluZSBuby1jb25kLWFzc2lnblxuXHRcdGlmICgobWF0Y2ggPSBlbnRpdHlDb2RlLm1hdGNoKC9eIyhcXGQrKSQvKSkpIHtcblx0XHRcdHJldHVybiBTdHJpbmcuZnJvbUNoYXJDb2RlKH5+bWF0Y2hbMV0pO1xuXHRcdH1cblx0XHRyZXR1cm4gZW50aXR5O1xuXHR9KTtcbn1cblxuZnVuY3Rpb24gdHJpbUJsYW5rTGluZXMoaW5wdXQ6IHN0cmluZyk6IHN0cmluZyB7XG5cdGxldCByZXN1bHQgPSBpbnB1dC5yZXBsYWNlKC9cXHIvZywgXCJcIik7XG5cblx0d2hpbGUgKHJlc3VsdC5jaGFyQXQoMCkgPT09IFwiXFxyXCIgfHwgcmVzdWx0LmNoYXJBdCgwKSA9PT0gXCJcXG5cIikge1xuXHRcdHJlc3VsdCA9IHJlc3VsdC5zbGljZSgxKTtcblx0fVxuXHR3aGlsZSAocmVzdWx0LnNsaWNlKC0xKSA9PT0gXCJcXHJcIiB8fCByZXN1bHQuc2xpY2UoLTEpID09PSBcIlxcblwiKSB7XG5cdFx0cmVzdWx0ID0gcmVzdWx0LnNsaWNlKDAsIC0xKTtcblx0fVxuXG5cdHJldHVybiByZXN1bHQ7XG59XG4iXSwKICAibWFwcGluZ3MiOiAiO0FBRUEsU0FBUyxpQkFBaUI7QUFFMUIsT0FBTyxRQUFRO0FBQ2YsT0FBTyxVQUFVO0FBRWpCLFNBQVMsY0FBYztBQUN2QixPQUFPLHlCQUE4QztBQUNyRCxTQUFTLHdCQUF3QjtBQUNqQyxTQUFTLG9CQUFvQjtBQUM3QixTQUFTLDBCQUEwQiwwQkFBMEI7QUFYN0QsSUFBTSxtQ0FBbUM7QUFhekMsSUFBTSxpQkFBaUIsS0FBSyxRQUFRLGdDQUFTO0FBRzdDLElBQU0sbUJBQW1CO0FBQUEsRUFDeEI7QUFBQSxFQUNBO0FBQUEsRUFDQTtBQUFBLEVBQ0E7QUFBQSxFQUNBO0FBQUEsRUFDQTtBQUFBLEVBQ0E7QUFBQSxFQUNBO0FBQUEsRUFDQTtBQUFBLEVBQ0E7QUFBQSxFQUNBO0FBQUEsRUFDQTtBQUFBLEVBQ0E7QUFBQSxFQUNBO0FBQ0Q7QUFHQSxJQUFPLHNCQUFRLGFBQWE7QUFBQSxFQUMzQixTQUFTO0FBQUEsSUFDUixPQUFPO0FBQUEsTUFDTixZQUFZLENBQUMsaUJBQWlCLENBQUM7QUFBQSxNQUMvQixPQUFPLFNBQVMsZ0JBQWdCO0FBRS9CLGNBQU0sYUFBYSxDQUFDLHVCQUF1QiwrQ0FBK0MsdUNBQXVDLDZDQUE2QztBQUM5SyxZQUFJLFdBQVcsU0FBUyxRQUFRLElBQUksRUFBRztBQUV2Qyx5QkFBaUIsT0FBTztBQUFBLE1BQ3pCO0FBQUEsSUFDRCxDQUFDO0FBQUEsSUFDRCxtQkFBbUIsQ0FBQyxpQkFBaUIsQ0FBQztBQUFBLEVBQ3ZDO0FBQUEsRUFDQSxTQUFTO0FBQUEsSUFDUixPQUFPO0FBQUEsTUFDTixFQUFFLE1BQU0saUNBQWlDLGFBQWEsS0FBSyxRQUFRLGdCQUFnQixRQUFRLEVBQUU7QUFBQSxNQUM3RixFQUFFLE1BQU0sc0JBQXNCLGFBQWEsZUFBZTtBQUFBLE1B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
