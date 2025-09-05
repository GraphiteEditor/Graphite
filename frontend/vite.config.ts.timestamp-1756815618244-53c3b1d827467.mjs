// vite.config.ts
import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";
import { svelte } from "file:///E:/projects/Graphite/frontend/node_modules/@sveltejs/vite-plugin-svelte/src/index.js";
import rollupPluginLicense from "file:///E:/projects/Graphite/frontend/node_modules/rollup-plugin-license/dist/index.js";
import { sveltePreprocess } from "file:///E:/projects/Graphite/frontend/node_modules/svelte-preprocess/dist/index.js";
import { defineConfig } from "file:///E:/projects/Graphite/frontend/node_modules/vite/dist/node/index.js";
import { DynamicPublicDirectory as viteMultipleAssets } from "file:///E:/projects/Graphite/frontend/node_modules/vite-multiple-assets/dist/index.mjs";
var __vite_injected_original_dirname = "E:\\projects\\Graphite\\frontend";
var projectRootDir = path.resolve(__vite_injected_original_dirname);
var vite_config_default = defineConfig({
  plugins: [
    svelte({
      preprocess: [sveltePreprocess()],
      onwarn(warning, defaultHandler) {
        const suppressed = [
          "css-unused-selector",
          // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
          "vite-plugin-svelte-css-no-scopable-elements",
          // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
          "a11y-no-static-element-interactions",
          // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
          "a11y-no-noninteractive-element-interactions",
          // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
          "a11y-click-events-have-key-events"
          // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
        ];
        if (suppressed.includes(warning.code)) return;
        defaultHandler?.(warning);
      }
    }),
    viteMultipleAssets(
      // Additional static asset directories besides `public/`
      [{ input: "../demo-artwork/**", output: "demo-artwork" }],
      // Options where we set custom MIME types
      { mimeTypes: { ".graphite": "application/json" } }
    )
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
            includePrivate: false,
            multipleVersions: true,
            allow: {
              test: `(${getAcceptedLicenses()})`,
              failOnUnlicensed: true,
              failOnViolation: true
            },
            output: {
              file: path.resolve(__vite_injected_original_dirname, "./dist/third-party-licenses.txt"),
              template: formatThirdPartyLicenses
            }
          }
        })
      ]
    }
  }
});
function formatThirdPartyLicenses(jsLicenses) {
  const rustLicenses = generateRustLicenses();
  const additionalLicenses = generateAdditionalLicenses();
  if (rustLicenses.length === 0) {
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
  let licenses = rustLicenses.concat(additionalLicenses);
  let foundLicensesIndex = void 0;
  let foundPackagesIndex = void 0;
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
    const matchedLicense = licenses.find(
      (license) => license.licenseName === licenseName && trimBlankLines(license.licenseText || "") === licenseText && trimBlankLines(license.noticeText || "") === noticeText
    );
    const pkg = { name, version, author, repository };
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
      let repo = repository;
      if (repo.startsWith("git+")) repo = repo.slice("git+".length);
      if (repo.startsWith("git://")) repo = repo.slice("git://".length);
      if (repo.endsWith(".git")) repo = repo.slice(0, -".git".length);
      if (repo.endsWith(".git#release")) repo = repo.slice(0, -".git#release".length);
      return `${name} ${version}${author ? ` - ${author}` : ""}${repo ? ` - ${repo}` : ""}`;
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
  formattedLicenseNotice += "\n";
  return formattedLicenseNotice;
}
function generateAdditionalLicenses() {
  const ADDITIONAL_LICENSES = [
    {
      licenseName: "SIL Open Font License 1.1",
      licenseTextPath: "node_modules/source-sans/LICENSE.txt",
      manifestPath: "node_modules/source-sans/package.json"
    },
    {
      licenseName: "SIL Open Font License 1.1",
      licenseTextPath: "node_modules/source-code-pro/LICENSE.md",
      manifestPath: "node_modules/source-code-pro/package.json"
    }
  ];
  return ADDITIONAL_LICENSES.map(({ licenseName, licenseTextPath, manifestPath }) => {
    const licenseText = fs.existsSync(licenseTextPath) && fs.readFileSync(licenseTextPath, "utf8") || "";
    const manifestJSON = fs.existsSync(manifestPath) && JSON.parse(fs.readFileSync(manifestPath, "utf8")) || {};
    const name = manifestJSON.name || "";
    const version = manifestJSON.version || "";
    const author = manifestJSON.author.name || manifestJSON.author || "";
    const repository = manifestJSON.repository?.url || "";
    return {
      licenseName,
      licenseText: trimBlankLines(licenseText),
      packages: [{ name, version, author, repository }]
    };
  });
}
function generateRustLicenses() {
  console.info("\n\nGenerating license information for Rust code\n");
  try {
    const { stdout, stderr, status } = spawnSync("cargo", ["about", "generate", "about.hbs"], {
      cwd: path.join(__vite_injected_original_dirname, ".."),
      encoding: "utf8",
      shell: true,
      windowsHide: true
      // Hide the terminal on Windows
    });
    if (status !== 0) {
      if (status !== 101) {
        console.error("cargo-about failed", status, stderr);
      }
      return [];
    }
    if (!stdout.trim().startsWith("GENERATED_BY_CARGO_ABOUT:")) {
      console.error("Unexpected output from cargo-about", stdout);
      return [];
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
    return [];
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
function getAcceptedLicenses() {
  const tomlContent = fs.readFileSync(path.resolve(__vite_injected_original_dirname, "../about.toml"), "utf8");
  const licensesBlock = tomlContent?.match(/accepted\s*=\s*\[([^\]]*)\]/)?.[1] || "";
  return licensesBlock.split("\n").map((line) => line.replace(/#.*$/, "")).join("\n").split(",").map((license) => license.trim().replace(/"/g, "")).filter((license) => license.length > 0).join(" OR ");
}
export {
  vite_config_default as default
};
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcudHMiXSwKICAic291cmNlc0NvbnRlbnQiOiBbImNvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9kaXJuYW1lID0gXCJFOlxcXFxwcm9qZWN0c1xcXFxHcmFwaGl0ZVxcXFxmcm9udGVuZFwiO2NvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9maWxlbmFtZSA9IFwiRTpcXFxccHJvamVjdHNcXFxcR3JhcGhpdGVcXFxcZnJvbnRlbmRcXFxcdml0ZS5jb25maWcudHNcIjtjb25zdCBfX3ZpdGVfaW5qZWN0ZWRfb3JpZ2luYWxfaW1wb3J0X21ldGFfdXJsID0gXCJmaWxlOi8vL0U6L3Byb2plY3RzL0dyYXBoaXRlL2Zyb250ZW5kL3ZpdGUuY29uZmlnLnRzXCI7LyogZXNsaW50LWRpc2FibGUgbm8tY29uc29sZSAqL1xuXG5pbXBvcnQgeyBzcGF3blN5bmMgfSBmcm9tIFwiY2hpbGRfcHJvY2Vzc1wiO1xuaW1wb3J0IGZzIGZyb20gXCJmc1wiO1xuaW1wb3J0IHBhdGggZnJvbSBcInBhdGhcIjtcblxuaW1wb3J0IHsgc3ZlbHRlIH0gZnJvbSBcIkBzdmVsdGVqcy92aXRlLXBsdWdpbi1zdmVsdGVcIjtcbmltcG9ydCByb2xsdXBQbHVnaW5MaWNlbnNlLCB7IHR5cGUgRGVwZW5kZW5jeSB9IGZyb20gXCJyb2xsdXAtcGx1Z2luLWxpY2Vuc2VcIjtcbmltcG9ydCB7IHN2ZWx0ZVByZXByb2Nlc3MgfSBmcm9tIFwic3ZlbHRlLXByZXByb2Nlc3NcIjtcbmltcG9ydCB7IGRlZmluZUNvbmZpZyB9IGZyb20gXCJ2aXRlXCI7XG5pbXBvcnQgeyBEeW5hbWljUHVibGljRGlyZWN0b3J5IGFzIHZpdGVNdWx0aXBsZUFzc2V0cyB9IGZyb20gXCJ2aXRlLW11bHRpcGxlLWFzc2V0c1wiO1xuXG5jb25zdCBwcm9qZWN0Um9vdERpciA9IHBhdGgucmVzb2x2ZShfX2Rpcm5hbWUpO1xuXG4vLyBodHRwczovL3ZpdGVqcy5kZXYvY29uZmlnL1xuZXhwb3J0IGRlZmF1bHQgZGVmaW5lQ29uZmlnKHtcblx0cGx1Z2luczogW1xuXHRcdHN2ZWx0ZSh7XG5cdFx0XHRwcmVwcm9jZXNzOiBbc3ZlbHRlUHJlcHJvY2VzcygpXSxcblx0XHRcdG9ud2Fybih3YXJuaW5nLCBkZWZhdWx0SGFuZGxlcikge1xuXHRcdFx0XHRjb25zdCBzdXBwcmVzc2VkID0gW1xuXHRcdFx0XHRcdFwiY3NzLXVudXNlZC1zZWxlY3RvclwiLCAvLyBOT1RJQ0U6IEtlZXAgdGhpcyBsaXN0IGluIHN5bmMgd2l0aCB0aGUgbGlzdCBpbiBgLnZzY29kZS9zZXR0aW5ncy5qc29uYFxuXHRcdFx0XHRcdFwidml0ZS1wbHVnaW4tc3ZlbHRlLWNzcy1uby1zY29wYWJsZS1lbGVtZW50c1wiLCAvLyBOT1RJQ0U6IEtlZXAgdGhpcyBsaXN0IGluIHN5bmMgd2l0aCB0aGUgbGlzdCBpbiBgLnZzY29kZS9zZXR0aW5ncy5qc29uYFxuXHRcdFx0XHRcdFwiYTExeS1uby1zdGF0aWMtZWxlbWVudC1pbnRlcmFjdGlvbnNcIiwgLy8gTk9USUNFOiBLZWVwIHRoaXMgbGlzdCBpbiBzeW5jIHdpdGggdGhlIGxpc3QgaW4gYC52c2NvZGUvc2V0dGluZ3MuanNvbmBcblx0XHRcdFx0XHRcImExMXktbm8tbm9uaW50ZXJhY3RpdmUtZWxlbWVudC1pbnRlcmFjdGlvbnNcIiwgLy8gTk9USUNFOiBLZWVwIHRoaXMgbGlzdCBpbiBzeW5jIHdpdGggdGhlIGxpc3QgaW4gYC52c2NvZGUvc2V0dGluZ3MuanNvbmBcblx0XHRcdFx0XHRcImExMXktY2xpY2stZXZlbnRzLWhhdmUta2V5LWV2ZW50c1wiLCAvLyBOT1RJQ0U6IEtlZXAgdGhpcyBsaXN0IGluIHN5bmMgd2l0aCB0aGUgbGlzdCBpbiBgLnZzY29kZS9zZXR0aW5ncy5qc29uYFxuXHRcdFx0XHRdO1xuXHRcdFx0XHRpZiAoc3VwcHJlc3NlZC5pbmNsdWRlcyh3YXJuaW5nLmNvZGUpKSByZXR1cm47XG5cblx0XHRcdFx0ZGVmYXVsdEhhbmRsZXI/Lih3YXJuaW5nKTtcblx0XHRcdH0sXG5cdFx0fSksXG5cdFx0dml0ZU11bHRpcGxlQXNzZXRzKFxuXHRcdFx0Ly8gQWRkaXRpb25hbCBzdGF0aWMgYXNzZXQgZGlyZWN0b3JpZXMgYmVzaWRlcyBgcHVibGljL2Bcblx0XHRcdFt7IGlucHV0OiBcIi4uL2RlbW8tYXJ0d29yay8qKlwiLCBvdXRwdXQ6IFwiZGVtby1hcnR3b3JrXCIgfV0sXG5cdFx0XHQvLyBPcHRpb25zIHdoZXJlIHdlIHNldCBjdXN0b20gTUlNRSB0eXBlc1xuXHRcdFx0eyBtaW1lVHlwZXM6IHsgXCIuZ3JhcGhpdGVcIjogXCJhcHBsaWNhdGlvbi9qc29uXCIgfSB9LFxuXHRcdCksXG5cdF0sXG5cdHJlc29sdmU6IHtcblx0XHRhbGlhczogW1xuXHRcdFx0eyBmaW5kOiAvQGdyYXBoaXRlLWZyb250ZW5kXFwvKC4qXFwuc3ZnKS8sIHJlcGxhY2VtZW50OiBwYXRoLnJlc29sdmUocHJvamVjdFJvb3REaXIsIFwiJDE/cmF3XCIpIH0sXG5cdFx0XHR7IGZpbmQ6IFwiQGdyYXBoaXRlLWZyb250ZW5kXCIsIHJlcGxhY2VtZW50OiBwcm9qZWN0Um9vdERpciB9LFxuXHRcdFx0eyBmaW5kOiBcIkBncmFwaGl0ZS8uLi9hc3NldHNcIiwgcmVwbGFjZW1lbnQ6IHBhdGgucmVzb2x2ZShwcm9qZWN0Um9vdERpciwgXCJhc3NldHNcIikgfSxcblx0XHRcdHsgZmluZDogXCJAZ3JhcGhpdGUvLi4vcHVibGljXCIsIHJlcGxhY2VtZW50OiBwYXRoLnJlc29sdmUocHJvamVjdFJvb3REaXIsIFwicHVibGljXCIpIH0sXG5cdFx0XHR7IGZpbmQ6IFwiQGdyYXBoaXRlXCIsIHJlcGxhY2VtZW50OiBwYXRoLnJlc29sdmUocHJvamVjdFJvb3REaXIsIFwic3JjXCIpIH0sXG5cdFx0XSxcblx0fSxcblx0c2VydmVyOiB7XG5cdFx0cG9ydDogODA4MCxcblx0XHRob3N0OiBcIjAuMC4wLjBcIixcblx0fSxcblx0YnVpbGQ6IHtcblx0XHRyb2xsdXBPcHRpb25zOiB7XG5cdFx0XHRwbHVnaW5zOiBbXG5cdFx0XHRcdHJvbGx1cFBsdWdpbkxpY2Vuc2Uoe1xuXHRcdFx0XHRcdHRoaXJkUGFydHk6IHtcblx0XHRcdFx0XHRcdGluY2x1ZGVQcml2YXRlOiBmYWxzZSxcblx0XHRcdFx0XHRcdG11bHRpcGxlVmVyc2lvbnM6IHRydWUsXG5cdFx0XHRcdFx0XHRhbGxvdzoge1xuXHRcdFx0XHRcdFx0XHR0ZXN0OiBgKCR7Z2V0QWNjZXB0ZWRMaWNlbnNlcygpfSlgLFxuXHRcdFx0XHRcdFx0XHRmYWlsT25VbmxpY2Vuc2VkOiB0cnVlLFxuXHRcdFx0XHRcdFx0XHRmYWlsT25WaW9sYXRpb246IHRydWUsXG5cdFx0XHRcdFx0XHR9LFxuXHRcdFx0XHRcdFx0b3V0cHV0OiB7XG5cdFx0XHRcdFx0XHRcdGZpbGU6IHBhdGgucmVzb2x2ZShfX2Rpcm5hbWUsIFwiLi9kaXN0L3RoaXJkLXBhcnR5LWxpY2Vuc2VzLnR4dFwiKSxcblx0XHRcdFx0XHRcdFx0dGVtcGxhdGU6IGZvcm1hdFRoaXJkUGFydHlMaWNlbnNlcyxcblx0XHRcdFx0XHRcdH0sXG5cdFx0XHRcdFx0fSxcblx0XHRcdFx0fSksXG5cdFx0XHRdLFxuXHRcdH0sXG5cdH0sXG59KTtcblxudHlwZSBMaWNlbnNlSW5mbyA9IHtcblx0bGljZW5zZU5hbWU6IHN0cmluZztcblx0bGljZW5zZVRleHQ6IHN0cmluZztcblx0bm90aWNlVGV4dD86IHN0cmluZztcblx0cGFja2FnZXM6IFBhY2thZ2VJbmZvW107XG59O1xuXG50eXBlIFBhY2thZ2VJbmZvID0ge1xuXHRuYW1lOiBzdHJpbmc7XG5cdHZlcnNpb246IHN0cmluZztcblx0YXV0aG9yOiBzdHJpbmc7XG5cdHJlcG9zaXRvcnk6IHN0cmluZztcbn07XG5cbmZ1bmN0aW9uIGZvcm1hdFRoaXJkUGFydHlMaWNlbnNlcyhqc0xpY2Vuc2VzOiBEZXBlbmRlbmN5W10pOiBzdHJpbmcge1xuXHQvLyBHZW5lcmF0ZSB0aGUgUnVzdCBsaWNlbnNlIGluZm9ybWF0aW9uLlxuXHRjb25zdCBydXN0TGljZW5zZXMgPSBnZW5lcmF0ZVJ1c3RMaWNlbnNlcygpO1xuXHRjb25zdCBhZGRpdGlvbmFsTGljZW5zZXMgPSBnZW5lcmF0ZUFkZGl0aW9uYWxMaWNlbnNlcygpO1xuXG5cdC8vIEVuc3VyZSB3ZSBoYXZlIHRoZSByZXF1aXJlZCBsaWNlbnNlIGluZm9ybWF0aW9uIHRvIHdvcmsgd2l0aCBiZWZvcmUgcHJvY2VlZGluZy5cblx0aWYgKHJ1c3RMaWNlbnNlcy5sZW5ndGggPT09IDApIHtcblx0XHQvLyBUaGlzIGlzIHByb2JhYmx5IGNhdXNlZCBieSBgY2FyZ28gYWJvdXRgIG5vdCBiZWluZyBpbnN0YWxsZWQuXG5cdFx0Y29uc29sZS5lcnJvcihcIkNvdWxkIG5vdCBydW4gYGNhcmdvIGFib3V0YCwgd2hpY2ggaXMgcmVxdWlyZWQgdG8gZ2VuZXJhdGUgbGljZW5zZSBpbmZvcm1hdGlvbi5cIik7XG5cdFx0Y29uc29sZS5lcnJvcihcIlRvIGluc3RhbGwgY2FyZ28tYWJvdXQgb24geW91ciBzeXN0ZW0sIHlvdSBjYW4gcnVuIGBjYXJnbyBpbnN0YWxsIGNhcmdvLWFib3V0YC5cIik7XG5cdFx0Y29uc29sZS5lcnJvcihcIkxpY2Vuc2UgaW5mb3JtYXRpb24gaXMgcmVxdWlyZWQgaW4gcHJvZHVjdGlvbiBidWlsZHMuIEFib3J0aW5nLlwiKTtcblxuXHRcdHByb2Nlc3MuZXhpdCgxKTtcblx0fVxuXHRpZiAoanNMaWNlbnNlcy5sZW5ndGggPT09IDApIHtcblx0XHRjb25zb2xlLmVycm9yKFwiTm8gSmF2YVNjcmlwdCBwYWNrYWdlIGxpY2Vuc2VzIHdlcmUgZm91bmQgYnkgYHJvbGx1cC1wbHVnaW4tbGljZW5zZWAuIFBsZWFzZSBpbnZlc3RpZ2F0ZS5cIik7XG5cdFx0Y29uc29sZS5lcnJvcihcIkxpY2Vuc2UgaW5mb3JtYXRpb24gaXMgcmVxdWlyZWQgaW4gcHJvZHVjdGlvbiBidWlsZHMuIEFib3J0aW5nLlwiKTtcblxuXHRcdHByb2Nlc3MuZXhpdCgxKTtcblx0fVxuXG5cdGxldCBsaWNlbnNlcyA9IHJ1c3RMaWNlbnNlcy5jb25jYXQoYWRkaXRpb25hbExpY2Vuc2VzKTtcblxuXHQvLyBTUEVDSUFMIENBU0U6IEZpbmQgdGhlbiBkdXBsaWNhdGUgdGhpcyBsaWNlbnNlIGlmIG9uZSBvZiBpdHMgcGFja2FnZXMgaXMgYHBhdGgtYm9vbGAsIGFkZGluZyBpdHMgbm90aWNlIHRleHQuXG5cdGxldCBmb3VuZExpY2Vuc2VzSW5kZXg6IG51bWJlciB8IHVuZGVmaW5lZCA9IHVuZGVmaW5lZDtcblx0bGV0IGZvdW5kUGFja2FnZXNJbmRleDogbnVtYmVyIHwgdW5kZWZpbmVkID0gdW5kZWZpbmVkO1xuXHRsaWNlbnNlcy5mb3JFYWNoKChsaWNlbnNlLCBsaWNlbnNlSW5kZXgpID0+IHtcblx0XHRsaWNlbnNlLnBhY2thZ2VzLmZvckVhY2goKHBrZywgcGtnSW5kZXgpID0+IHtcblx0XHRcdGlmIChwa2cubmFtZSA9PT0gXCJwYXRoLWJvb2xcIikge1xuXHRcdFx0XHRmb3VuZExpY2Vuc2VzSW5kZXggPSBsaWNlbnNlSW5kZXg7XG5cdFx0XHRcdGZvdW5kUGFja2FnZXNJbmRleCA9IHBrZ0luZGV4O1xuXHRcdFx0fVxuXHRcdH0pO1xuXHR9KTtcblx0aWYgKGZvdW5kTGljZW5zZXNJbmRleCAhPT0gdW5kZWZpbmVkICYmIGZvdW5kUGFja2FnZXNJbmRleCAhPT0gdW5kZWZpbmVkKSB7XG5cdFx0Y29uc3QgbGljZW5zZSA9IGxpY2Vuc2VzW2ZvdW5kTGljZW5zZXNJbmRleF07XG5cdFx0Y29uc3QgcGtnID0gbGljZW5zZS5wYWNrYWdlc1tmb3VuZFBhY2thZ2VzSW5kZXhdO1xuXG5cdFx0bGljZW5zZS5wYWNrYWdlcyA9IGxpY2Vuc2UucGFja2FnZXMuZmlsdGVyKChwa2cpID0+IHBrZy5uYW1lICE9PSBcInBhdGgtYm9vbFwiKTtcblx0XHRjb25zdCBub3RpY2VUZXh0ID0gZnMucmVhZEZpbGVTeW5jKHBhdGgucmVzb2x2ZShfX2Rpcm5hbWUsIFwiLi4vbGlicmFyaWVzL3BhdGgtYm9vbC9OT1RJQ0VcIiksIFwidXRmOFwiKTtcblxuXHRcdGxpY2Vuc2VzLnB1c2goe1xuXHRcdFx0bGljZW5zZU5hbWU6IGxpY2Vuc2UubGljZW5zZU5hbWUsXG5cdFx0XHRsaWNlbnNlVGV4dDogbGljZW5zZS5saWNlbnNlVGV4dCxcblx0XHRcdG5vdGljZVRleHQsXG5cdFx0XHRwYWNrYWdlczogW3BrZ10sXG5cdFx0fSk7XG5cdH1cblxuXHQvLyBFeHRlbmQgdGhlIGxpY2Vuc2UgbGlzdCB3aXRoIHRoZSBwcm92aWRlZCBKUyBsaWNlbnNlcy5cblx0anNMaWNlbnNlcy5mb3JFYWNoKChqc0xpY2Vuc2UpID0+IHtcblx0XHRjb25zdCBuYW1lID0ganNMaWNlbnNlLm5hbWUgfHwgXCJcIjtcblx0XHRjb25zdCB2ZXJzaW9uID0ganNMaWNlbnNlLnZlcnNpb24gfHwgXCJcIjtcblx0XHRjb25zdCBhdXRob3IgPSBqc0xpY2Vuc2UuYXV0aG9yPy50ZXh0KCkgfHwgXCJcIjtcblx0XHRjb25zdCBsaWNlbnNlTmFtZSA9IGpzTGljZW5zZS5saWNlbnNlIHx8IFwiXCI7XG5cdFx0Y29uc3QgbGljZW5zZVRleHQgPSB0cmltQmxhbmtMaW5lcyhqc0xpY2Vuc2UubGljZW5zZVRleHQgfHwgXCJcIik7XG5cdFx0Y29uc3Qgbm90aWNlVGV4dCA9IHRyaW1CbGFua0xpbmVzKGpzTGljZW5zZS5ub3RpY2VUZXh0IHx8IFwiXCIpO1xuXG5cdFx0bGV0IHJlcG9zaXRvcnkgPSBqc0xpY2Vuc2UucmVwb3NpdG9yeSB8fCBcIlwiO1xuXHRcdGlmIChyZXBvc2l0b3J5ICYmIHR5cGVvZiByZXBvc2l0b3J5ID09PSBcIm9iamVjdFwiKSByZXBvc2l0b3J5ID0gcmVwb3NpdG9yeS51cmw7XG5cblx0XHRjb25zdCBtYXRjaGVkTGljZW5zZSA9IGxpY2Vuc2VzLmZpbmQoXG5cdFx0XHQobGljZW5zZSkgPT4gbGljZW5zZS5saWNlbnNlTmFtZSA9PT0gbGljZW5zZU5hbWUgJiYgdHJpbUJsYW5rTGluZXMobGljZW5zZS5saWNlbnNlVGV4dCB8fCBcIlwiKSA9PT0gbGljZW5zZVRleHQgJiYgdHJpbUJsYW5rTGluZXMobGljZW5zZS5ub3RpY2VUZXh0IHx8IFwiXCIpID09PSBub3RpY2VUZXh0LFxuXHRcdCk7XG5cblx0XHRjb25zdCBwa2c6IFBhY2thZ2VJbmZvID0geyBuYW1lLCB2ZXJzaW9uLCBhdXRob3IsIHJlcG9zaXRvcnkgfTtcblx0XHRpZiAobWF0Y2hlZExpY2Vuc2UpIG1hdGNoZWRMaWNlbnNlLnBhY2thZ2VzLnB1c2gocGtnKTtcblx0XHRlbHNlIGxpY2Vuc2VzLnB1c2goeyBsaWNlbnNlTmFtZSwgbGljZW5zZVRleHQsIG5vdGljZVRleHQsIHBhY2thZ2VzOiBbcGtnXSB9KTtcblx0fSk7XG5cblx0Ly8gQ29tYmluZSBhbnkgbGljZW5zZSBub3RpY2VzIGludG8gdGhlIGxpY2Vuc2UgdGV4dC5cblx0bGljZW5zZXMuZm9yRWFjaCgobGljZW5zZSwgaW5kZXgpID0+IHtcblx0XHRpZiAobGljZW5zZS5ub3RpY2VUZXh0KSB7XG5cdFx0XHRsaWNlbnNlc1tpbmRleF0ubGljZW5zZVRleHQgKz0gXCJcXG5cXG5cIjtcblx0XHRcdGxpY2Vuc2VzW2luZGV4XS5saWNlbnNlVGV4dCArPSBcIiBfX19fX19fX19fX19fX19fX19fX19fX19fX19fX19fX19fX19fX19cXG5cIjtcblx0XHRcdGxpY2Vuc2VzW2luZGV4XS5saWNlbnNlVGV4dCArPSBcIlx1MjUwMiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIFx1MjUwMlxcblwiO1xuXHRcdFx0bGljZW5zZXNbaW5kZXhdLmxpY2Vuc2VUZXh0ICs9IFwiXHUyNTAyIFRIRSBGT0xMT1dJTkcgTk9USUNFIEZJTEUgSVMgSU5DTFVERUQgXHUyNTAyXFxuXCI7XG5cdFx0XHRsaWNlbnNlc1tpbmRleF0ubGljZW5zZVRleHQgKz0gXCJcdTI1MDIgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICBcdTI1MDJcXG5cIjtcblx0XHRcdGxpY2Vuc2VzW2luZGV4XS5saWNlbnNlVGV4dCArPSBcIiBcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcdTIwM0VcXG5cXG5cIjtcblx0XHRcdGxpY2Vuc2VzW2luZGV4XS5saWNlbnNlVGV4dCArPSBgJHtsaWNlbnNlLm5vdGljZVRleHR9XFxuYDtcblx0XHRcdGxpY2Vuc2VzW2luZGV4XS5ub3RpY2VUZXh0ID0gdW5kZWZpbmVkO1xuXHRcdH1cblx0fSk7XG5cblx0Ly8gRGUtZHVwbGljYXRlIGFueSBsaWNlbnNlcyB3aXRoIHRoZSBzYW1lIHRleHQgYnkgbWVyZ2luZyB0aGVpciBsaXN0cyBvZiBwYWNrYWdlcy5cblx0Y29uc3QgbGljZW5zZXNOb3JtYWxpemVkV2hpdGVzcGFjZSA9IGxpY2Vuc2VzLm1hcCgobGljZW5zZSkgPT4gbGljZW5zZS5saWNlbnNlVGV4dC5yZXBsYWNlKC9bXFxuXFxzXSsvZywgXCIgXCIpLnRyaW0oKSk7XG5cdGxpY2Vuc2VzLmZvckVhY2goKGN1cnJlbnRMaWNlbnNlLCBjdXJyZW50TGljZW5zZUluZGV4KSA9PiB7XG5cdFx0bGljZW5zZXMuc2xpY2UoMCwgY3VycmVudExpY2Vuc2VJbmRleCkuZm9yRWFjaCgoY29tcGFyaXNvbkxpY2Vuc2UsIGNvbXBhcmlzb25MaWNlbnNlSW5kZXgpID0+IHtcblx0XHRcdGlmIChsaWNlbnNlc05vcm1hbGl6ZWRXaGl0ZXNwYWNlW2N1cnJlbnRMaWNlbnNlSW5kZXhdID09PSBsaWNlbnNlc05vcm1hbGl6ZWRXaGl0ZXNwYWNlW2NvbXBhcmlzb25MaWNlbnNlSW5kZXhdKSB7XG5cdFx0XHRcdGN1cnJlbnRMaWNlbnNlLnBhY2thZ2VzLnB1c2goLi4uY29tcGFyaXNvbkxpY2Vuc2UucGFja2FnZXMpO1xuXHRcdFx0XHRjb21wYXJpc29uTGljZW5zZS5wYWNrYWdlcyA9IFtdO1xuXHRcdFx0XHQvLyBBZnRlciBlbXB0eWluZyB0aGUgcGFja2FnZXMsIHRoZSByZWR1bmRhbnQgbGljZW5zZSB3aXRoIG5vIHBhY2thZ2VzIHdpbGwgYmUgcmVtb3ZlZCBpbiB0aGUgbmV4dCBzdGVwJ3MgYGZpbHRlcigpYC5cblx0XHRcdH1cblx0XHR9KTtcblx0fSk7XG5cblx0Ly8gRmlsdGVyIG91dCBmaXJzdC1wYXJ0eSBpbnRlcm5hbCBHcmFwaGl0ZSBjcmF0ZXMuXG5cdGxpY2Vuc2VzID0gbGljZW5zZXMuZmlsdGVyKChsaWNlbnNlKSA9PiB7XG5cdFx0bGljZW5zZS5wYWNrYWdlcyA9IGxpY2Vuc2UucGFja2FnZXMuZmlsdGVyKFxuXHRcdFx0KHBhY2thZ2VJbmZvKSA9PlxuXHRcdFx0XHQhKHBhY2thZ2VJbmZvLnJlcG9zaXRvcnkgJiYgcGFja2FnZUluZm8ucmVwb3NpdG9yeS50b0xvd2VyQ2FzZSgpLmluY2x1ZGVzKFwiZ2l0aHViLmNvbS9HcmFwaGl0ZUVkaXRvci9HcmFwaGl0ZVwiLnRvTG93ZXJDYXNlKCkpKSAmJlxuXHRcdFx0XHQhKFxuXHRcdFx0XHRcdHBhY2thZ2VJbmZvLmF1dGhvciAmJlxuXHRcdFx0XHRcdHBhY2thZ2VJbmZvLmF1dGhvci50b0xvd2VyQ2FzZSgpLmluY2x1ZGVzKFwiY29udGFjdEBncmFwaGl0ZS5yc1wiKSAmJlxuXHRcdFx0XHRcdC8vIEV4Y2x1ZGUgYSBjb21tYSB3aGljaCBpbmRpY2F0ZXMgbXVsdGlwbGUgYXV0aG9ycywgd2hpY2ggd2UgbmVlZCB0byBub3QgZmlsdGVyIG91dFxuXHRcdFx0XHRcdCFwYWNrYWdlSW5mby5hdXRob3IudG9Mb3dlckNhc2UoKS5pbmNsdWRlcyhcIixcIilcblx0XHRcdFx0KSxcblx0XHQpO1xuXHRcdHJldHVybiBsaWNlbnNlLnBhY2thZ2VzLmxlbmd0aCA+IDA7XG5cdH0pO1xuXG5cdC8vIFNvcnQgdGhlIGxpY2Vuc2VzIGJ5IHRoZSBudW1iZXIgb2YgcGFja2FnZXMgdXNpbmcgdGhlIHNhbWUgbGljZW5zZSwgYW5kIHRoZW4gYWxwaGFiZXRpY2FsbHkgYnkgbGljZW5zZSBuYW1lLlxuXHRsaWNlbnNlcy5zb3J0KChhLCBiKSA9PiBhLmxpY2Vuc2VUZXh0LmxvY2FsZUNvbXBhcmUoYi5saWNlbnNlVGV4dCkpO1xuXHRsaWNlbnNlcy5zb3J0KChhLCBiKSA9PiBhLmxpY2Vuc2VOYW1lLmxvY2FsZUNvbXBhcmUoYi5saWNlbnNlTmFtZSkpO1xuXHRsaWNlbnNlcy5zb3J0KChhLCBiKSA9PiBiLnBhY2thZ2VzLmxlbmd0aCAtIGEucGFja2FnZXMubGVuZ3RoKTtcblx0Ly8gU29ydCB0aGUgaW5kaXZpZHVhbCBwYWNrYWdlcyB1c2luZyBlYWNoIGxpY2Vuc2UgYWxwaGFiZXRpY2FsbHkuXG5cdGxpY2Vuc2VzLmZvckVhY2goKGxpY2Vuc2UpID0+IHtcblx0XHRsaWNlbnNlLnBhY2thZ2VzLnNvcnQoKGEsIGIpID0+IGEubmFtZS5sb2NhbGVDb21wYXJlKGIubmFtZSkpO1xuXHR9KTtcblxuXHQvLyBQcmVwYXJlIGEgaGVhZGVyIGZvciB0aGUgbGljZW5zZSBub3RpY2UuXG5cdGxldCBmb3JtYXR0ZWRMaWNlbnNlTm90aWNlID0gXCJcIjtcblx0Zm9ybWF0dGVkTGljZW5zZU5vdGljZSArPSBcIlx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFx1MjU5MFxcblwiO1xuXHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IFwiXHUyNTkwXHUyNTkwICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgXHUyNTkwXHUyNTkwXFxuXCI7XG5cdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gXCJcdTI1OTBcdTI1OTAgICBHUkFQSElURSBUSElSRC1QQVJUWSBTT0ZUV0FSRSBMSUNFTlNFIE5PVElDRVMgICBcdTI1OTBcdTI1OTBcXG5cIjtcblx0Zm9ybWF0dGVkTGljZW5zZU5vdGljZSArPSBcIlx1MjU5MFx1MjU5MCAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIFx1MjU5MFx1MjU5MFxcblwiO1xuXHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IFwiXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXHUyNTkwXFxuXCI7XG5cblx0Ly8gQXBwZW5kIGEgYmxvY2sgZm9yIGVhY2ggbGljZW5zZSBzaGFyZWQgYnkgbXVsdGlwbGUgcGFja2FnZXMgd2l0aCBpZGVudGljYWwgbGljZW5zZSB0ZXh0LlxuXHRsaWNlbnNlcy5mb3JFYWNoKChsaWNlbnNlKSA9PiB7XG5cdFx0bGV0IHBhY2thZ2VzV2l0aFNhbWVMaWNlbnNlID0gbGljZW5zZS5wYWNrYWdlcy5tYXAoKHBhY2thZ2VJbmZvKSA9PiB7XG5cdFx0XHRjb25zdCB7IG5hbWUsIHZlcnNpb24sIGF1dGhvciwgcmVwb3NpdG9yeSB9ID0gcGFja2FnZUluZm87XG5cblx0XHRcdC8vIFJlbW92ZSB0aGUgYGdpdCtgIG9yIGBnaXQ6Ly9gIHByZWZpeCBhbmQgYC5naXRgIHN1ZmZpeC5cblx0XHRcdGxldCByZXBvID0gcmVwb3NpdG9yeTtcblx0XHRcdGlmIChyZXBvLnN0YXJ0c1dpdGgoXCJnaXQrXCIpKSByZXBvID0gcmVwby5zbGljZShcImdpdCtcIi5sZW5ndGgpO1xuXHRcdFx0aWYgKHJlcG8uc3RhcnRzV2l0aChcImdpdDovL1wiKSkgcmVwbyA9IHJlcG8uc2xpY2UoXCJnaXQ6Ly9cIi5sZW5ndGgpO1xuXHRcdFx0aWYgKHJlcG8uZW5kc1dpdGgoXCIuZ2l0XCIpKSByZXBvID0gcmVwby5zbGljZSgwLCAtXCIuZ2l0XCIubGVuZ3RoKTtcblx0XHRcdGlmIChyZXBvLmVuZHNXaXRoKFwiLmdpdCNyZWxlYXNlXCIpKSByZXBvID0gcmVwby5zbGljZSgwLCAtXCIuZ2l0I3JlbGVhc2VcIi5sZW5ndGgpO1xuXG5cdFx0XHRyZXR1cm4gYCR7bmFtZX0gJHt2ZXJzaW9ufSR7YXV0aG9yID8gYCAtICR7YXV0aG9yfWAgOiBcIlwifSR7cmVwbyA/IGAgLSAke3JlcG99YCA6IFwiXCJ9YDtcblx0XHR9KTtcblx0XHRjb25zdCBtdWx0aSA9IHBhY2thZ2VzV2l0aFNhbWVMaWNlbnNlLmxlbmd0aCAhPT0gMTtcblx0XHRjb25zdCBzYXlzTGljZW5zZSA9IGxpY2Vuc2UubGljZW5zZU5hbWUudG9Mb3dlckNhc2UoKS5pbmNsdWRlcyhcImxpY2Vuc2VcIik7XG5cdFx0Y29uc3QgaGVhZGVyID0gYFRoZSBwYWNrYWdlJHttdWx0aSA/IFwic1wiIDogXCJcIn0gbGlzdGVkIGhlcmUgJHttdWx0aSA/IFwiYXJlXCIgOiBcImlzXCJ9IGxpY2Vuc2VkIHVuZGVyIHRoZSB0ZXJtcyBvZiB0aGUgJHtsaWNlbnNlLmxpY2Vuc2VOYW1lfSR7c2F5c0xpY2Vuc2UgPyBcIlwiIDogXCIgbGljZW5zZVwifSBwcmludGVkIGJlbmVhdGhgO1xuXHRcdGNvbnN0IHBhY2thZ2VzTGluZUxlbmd0aCA9IE1hdGgubWF4KGhlYWRlci5sZW5ndGgsIC4uLnBhY2thZ2VzV2l0aFNhbWVMaWNlbnNlLm1hcCgobGluZSkgPT4gbGluZS5sZW5ndGgpKTtcblx0XHRwYWNrYWdlc1dpdGhTYW1lTGljZW5zZSA9IHBhY2thZ2VzV2l0aFNhbWVMaWNlbnNlLm1hcCgobGluZSkgPT4gYFx1MjUwMiAke2xpbmV9JHtcIiBcIi5yZXBlYXQocGFja2FnZXNMaW5lTGVuZ3RoIC0gbGluZS5sZW5ndGgpfSBcdTI1MDJgKTtcblxuXHRcdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gXCJcXG5cIjtcblx0XHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IGAgJHtcIl9cIi5yZXBlYXQocGFja2FnZXNMaW5lTGVuZ3RoICsgMil9XFxuYDtcblx0XHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IGBcdTI1MDIgJHtcIiBcIi5yZXBlYXQocGFja2FnZXNMaW5lTGVuZ3RoKX0gXHUyNTAyXFxuYDtcblx0XHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IGBcdTI1MDIgJHtoZWFkZXJ9JHtcIiBcIi5yZXBlYXQocGFja2FnZXNMaW5lTGVuZ3RoIC0gaGVhZGVyLmxlbmd0aCl9IFx1MjUwMlxcbmA7XG5cdFx0Zm9ybWF0dGVkTGljZW5zZU5vdGljZSArPSBgXHUyNTAyJHtcIl9cIi5yZXBlYXQocGFja2FnZXNMaW5lTGVuZ3RoICsgMil9XHUyNTAyXFxuYDtcblx0XHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IGAke3BhY2thZ2VzV2l0aFNhbWVMaWNlbnNlLmpvaW4oXCJcXG5cIil9XFxuYDtcblx0XHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IGAgJHtcIlx1MjAzRVwiLnJlcGVhdChwYWNrYWdlc0xpbmVMZW5ndGggKyAyKX1cXG5gO1xuXHRcdGZvcm1hdHRlZExpY2Vuc2VOb3RpY2UgKz0gYCR7bGljZW5zZS5saWNlbnNlVGV4dH1cXG5gO1xuXHR9KTtcblxuXHRmb3JtYXR0ZWRMaWNlbnNlTm90aWNlICs9IFwiXFxuXCI7XG5cdHJldHVybiBmb3JtYXR0ZWRMaWNlbnNlTm90aWNlO1xufVxuXG4vLyBJbmNsdWRlIGFkZGl0aW9uYWwgbGljZW5zZXMgdGhhdCBhcmVuJ3QgYXV0b21hdGljYWxseSBnZW5lcmF0ZWQgYnkgYGNhcmdvIGFib3V0YCBvciBgcm9sbHVwLXBsdWdpbi1saWNlbnNlYC5cbmZ1bmN0aW9uIGdlbmVyYXRlQWRkaXRpb25hbExpY2Vuc2VzKCk6IExpY2Vuc2VJbmZvW10ge1xuXHRjb25zdCBBRERJVElPTkFMX0xJQ0VOU0VTID0gW1xuXHRcdHtcblx0XHRcdGxpY2Vuc2VOYW1lOiBcIlNJTCBPcGVuIEZvbnQgTGljZW5zZSAxLjFcIixcblx0XHRcdGxpY2Vuc2VUZXh0UGF0aDogXCJub2RlX21vZHVsZXMvc291cmNlLXNhbnMvTElDRU5TRS50eHRcIixcblx0XHRcdG1hbmlmZXN0UGF0aDogXCJub2RlX21vZHVsZXMvc291cmNlLXNhbnMvcGFja2FnZS5qc29uXCIsXG5cdFx0fSxcblx0XHR7XG5cdFx0XHRsaWNlbnNlTmFtZTogXCJTSUwgT3BlbiBGb250IExpY2Vuc2UgMS4xXCIsXG5cdFx0XHRsaWNlbnNlVGV4dFBhdGg6IFwibm9kZV9tb2R1bGVzL3NvdXJjZS1jb2RlLXByby9MSUNFTlNFLm1kXCIsXG5cdFx0XHRtYW5pZmVzdFBhdGg6IFwibm9kZV9tb2R1bGVzL3NvdXJjZS1jb2RlLXByby9wYWNrYWdlLmpzb25cIixcblx0XHR9LFxuXHRdO1xuXG5cdHJldHVybiBBRERJVElPTkFMX0xJQ0VOU0VTLm1hcCgoeyBsaWNlbnNlTmFtZSwgbGljZW5zZVRleHRQYXRoLCBtYW5pZmVzdFBhdGggfSkgPT4ge1xuXHRcdGNvbnN0IGxpY2Vuc2VUZXh0ID0gKGZzLmV4aXN0c1N5bmMobGljZW5zZVRleHRQYXRoKSAmJiBmcy5yZWFkRmlsZVN5bmMobGljZW5zZVRleHRQYXRoLCBcInV0ZjhcIikpIHx8IFwiXCI7XG5cblx0XHRjb25zdCBtYW5pZmVzdEpTT04gPSAoZnMuZXhpc3RzU3luYyhtYW5pZmVzdFBhdGgpICYmIEpTT04ucGFyc2UoZnMucmVhZEZpbGVTeW5jKG1hbmlmZXN0UGF0aCwgXCJ1dGY4XCIpKSkgfHwge307XG5cdFx0Y29uc3QgbmFtZSA9IG1hbmlmZXN0SlNPTi5uYW1lIHx8IFwiXCI7XG5cdFx0Y29uc3QgdmVyc2lvbiA9IG1hbmlmZXN0SlNPTi52ZXJzaW9uIHx8IFwiXCI7XG5cdFx0Y29uc3QgYXV0aG9yID0gbWFuaWZlc3RKU09OLmF1dGhvci5uYW1lIHx8IG1hbmlmZXN0SlNPTi5hdXRob3IgfHwgXCJcIjtcblx0XHRjb25zdCByZXBvc2l0b3J5ID0gbWFuaWZlc3RKU09OLnJlcG9zaXRvcnk/LnVybCB8fCBcIlwiO1xuXG5cdFx0cmV0dXJuIHtcblx0XHRcdGxpY2Vuc2VOYW1lLFxuXHRcdFx0bGljZW5zZVRleHQ6IHRyaW1CbGFua0xpbmVzKGxpY2Vuc2VUZXh0KSxcblx0XHRcdHBhY2thZ2VzOiBbeyBuYW1lLCB2ZXJzaW9uLCBhdXRob3IsIHJlcG9zaXRvcnkgfV0sXG5cdFx0fTtcblx0fSk7XG59XG5cbmZ1bmN0aW9uIGdlbmVyYXRlUnVzdExpY2Vuc2VzKCk6IExpY2Vuc2VJbmZvW10ge1xuXHQvLyBMb2cgdGhlIHN0YXJ0aW5nIHN0YXR1cyB0byB0aGUgYnVpbGQgb3V0cHV0LlxuXHRjb25zb2xlLmluZm8oXCJcXG5cXG5HZW5lcmF0aW5nIGxpY2Vuc2UgaW5mb3JtYXRpb24gZm9yIFJ1c3QgY29kZVxcblwiKTtcblxuXHR0cnkge1xuXHRcdC8vIENhbGwgYGNhcmdvIGFib3V0YCBpbiB0aGUgdGVybWluYWwgdG8gZ2VuZXJhdGUgdGhlIGxpY2Vuc2UgaW5mb3JtYXRpb24gZm9yIFJ1c3QgY3JhdGVzLlxuXHRcdC8vIFRoZSBgYWJvdXQuaGJzYCBmaWxlIGlzIHdyaXR0ZW4gc28gaXQgZ2VuZXJhdGVzIGEgdmFsaWQgSmF2YVNjcmlwdCBhcnJheSBleHByZXNzaW9uIHdoaWNoIHdlIGV2YWx1YXRlIGJlbG93LlxuXHRcdGNvbnN0IHsgc3Rkb3V0LCBzdGRlcnIsIHN0YXR1cyB9ID0gc3Bhd25TeW5jKFwiY2FyZ29cIiwgW1wiYWJvdXRcIiwgXCJnZW5lcmF0ZVwiLCBcImFib3V0Lmhic1wiXSwge1xuXHRcdFx0Y3dkOiBwYXRoLmpvaW4oX19kaXJuYW1lLCBcIi4uXCIpLFxuXHRcdFx0ZW5jb2Rpbmc6IFwidXRmOFwiLFxuXHRcdFx0c2hlbGw6IHRydWUsXG5cdFx0XHR3aW5kb3dzSGlkZTogdHJ1ZSwgLy8gSGlkZSB0aGUgdGVybWluYWwgb24gV2luZG93c1xuXHRcdH0pO1xuXG5cdFx0Ly8gSWYgdGhlIGNvbW1hbmQgZmFpbGVkLCBwcmludCB0aGUgZXJyb3IgbWVzc2FnZSBhbmQgZXhpdCBlYXJseS5cblx0XHRpZiAoc3RhdHVzICE9PSAwKSB7XG5cdFx0XHQvLyBDYXJnbyByZXR1cm5zIDEwMSB3aGVuIHRoZSBzdWJjb21tYW5kIChgYWJvdXRgKSB3YXNuJ3QgZm91bmQsIHNvIHdlIHNraXAgcHJpbnRpbmcgdGhlIGJlbG93IGVycm9yIG1lc3NhZ2UgaW4gdGhhdCBjYXNlLlxuXHRcdFx0aWYgKHN0YXR1cyAhPT0gMTAxKSB7XG5cdFx0XHRcdGNvbnNvbGUuZXJyb3IoXCJjYXJnby1hYm91dCBmYWlsZWRcIiwgc3RhdHVzLCBzdGRlcnIpO1xuXHRcdFx0fVxuXHRcdFx0cmV0dXJuIFtdO1xuXHRcdH1cblxuXHRcdC8vIE1ha2Ugc3VyZSB0aGUgb3V0cHV0IHN0YXJ0cyB3aXRoIHRoaXMgZXhwZWN0ZWQgbGFiZWwsIHdoaWNoIGxldHMgdXMga25vdyB0aGUgZmlsZSBnZW5lcmF0ZWQgd2l0aCBleHBlY3RlZCBvdXRwdXQuXG5cdFx0Ly8gV2UgZG9uJ3Qgd2FudCB0byBldmFsIGFuIGVycm9yIG1lc3NhZ2Ugb3Igc29tZXRoaW5nIGVsc2UsIHNvIHdlIGZhaWwgZWFybHkgaWYgdGhhdCBoYXBwZW5zLlxuXHRcdGlmICghc3Rkb3V0LnRyaW0oKS5zdGFydHNXaXRoKFwiR0VORVJBVEVEX0JZX0NBUkdPX0FCT1VUOlwiKSkge1xuXHRcdFx0Y29uc29sZS5lcnJvcihcIlVuZXhwZWN0ZWQgb3V0cHV0IGZyb20gY2FyZ28tYWJvdXRcIiwgc3Rkb3V0KTtcblx0XHRcdHJldHVybiBbXTtcblx0XHR9XG5cblx0XHQvLyBDb252ZXJ0IHRoZSBhcnJheSBKUyBzeW50YXggc3RyaW5nIGludG8gYW4gYWN0dWFsIEpTIGFycmF5IGluIG1lbW9yeS5cblx0XHQvLyBTZWN1cml0eS13aXNlLCBldmFsKCkgaXNuJ3QgYW55IHdvcnNlIHRoYW4gcmVxdWlyZSgpLCBidXQgaXQncyBhYmxlIHRvIHdvcmsgd2l0aG91dCBhIHRlbXBvcmFyeSBmaWxlLlxuXHRcdC8vIFdlIGNhbGwgZXZhbCBpbmRpcmVjdGx5IHRvIGF2b2lkIGEgd2FybmluZyBhcyBleHBsYWluZWQgaGVyZTogPGh0dHBzOi8vZXNidWlsZC5naXRodWIuaW8vY29udGVudC10eXBlcy8jZGlyZWN0LWV2YWw+LlxuXHRcdGNvbnN0IGluZGlyZWN0RXZhbCA9IGV2YWw7XG5cdFx0Y29uc3QgbGljZW5zZXNBcnJheSA9IGluZGlyZWN0RXZhbChzdGRvdXQpIGFzIExpY2Vuc2VJbmZvW107XG5cblx0XHQvLyBSZW1vdmUgdGhlIEhUTUwgY2hhcmFjdGVyIGVuY29kaW5nIGNhdXNlZCBieSBIYW5kbGViYXJzLlxuXHRcdGNvbnN0IHJ1c3RMaWNlbnNlcyA9IChsaWNlbnNlc0FycmF5IHx8IFtdKS5tYXAoXG5cdFx0XHQocnVzdExpY2Vuc2UpOiBMaWNlbnNlSW5mbyA9PiAoe1xuXHRcdFx0XHRsaWNlbnNlTmFtZTogaHRtbERlY29kZShydXN0TGljZW5zZS5saWNlbnNlTmFtZSksXG5cdFx0XHRcdGxpY2Vuc2VUZXh0OiB0cmltQmxhbmtMaW5lcyhodG1sRGVjb2RlKHJ1c3RMaWNlbnNlLmxpY2Vuc2VUZXh0KSksXG5cdFx0XHRcdHBhY2thZ2VzOiBydXN0TGljZW5zZS5wYWNrYWdlcy5tYXAoXG5cdFx0XHRcdFx0KHBhY2thZ2VJbmZvKTogUGFja2FnZUluZm8gPT4gKHtcblx0XHRcdFx0XHRcdG5hbWU6IGh0bWxEZWNvZGUocGFja2FnZUluZm8ubmFtZSksXG5cdFx0XHRcdFx0XHR2ZXJzaW9uOiBodG1sRGVjb2RlKHBhY2thZ2VJbmZvLnZlcnNpb24pLFxuXHRcdFx0XHRcdFx0YXV0aG9yOiBodG1sRGVjb2RlKHBhY2thZ2VJbmZvLmF1dGhvcilcblx0XHRcdFx0XHRcdFx0LnJlcGxhY2UoL1xcWyguKiksIFxcXS8sIFwiJDFcIilcblx0XHRcdFx0XHRcdFx0LnJlcGxhY2UoXCJbXVwiLCBcIlwiKSxcblx0XHRcdFx0XHRcdHJlcG9zaXRvcnk6IGh0bWxEZWNvZGUocGFja2FnZUluZm8ucmVwb3NpdG9yeSksXG5cdFx0XHRcdFx0fSksXG5cdFx0XHRcdCksXG5cdFx0XHR9KSxcblx0XHQpO1xuXG5cdFx0cmV0dXJuIHJ1c3RMaWNlbnNlcztcblx0fSBjYXRjaCAoXykge1xuXHRcdHJldHVybiBbXTtcblx0fVxufVxuXG5mdW5jdGlvbiBodG1sRGVjb2RlKGlucHV0OiBzdHJpbmcpOiBzdHJpbmcge1xuXHRpZiAoIWlucHV0KSByZXR1cm4gaW5wdXQ7XG5cblx0Y29uc3QgaHRtbEVudGl0aWVzID0ge1xuXHRcdG5ic3A6IFwiIFwiLFxuXHRcdGNvcHk6IFwiXHUwMEE5XCIsXG5cdFx0cmVnOiBcIlx1MDBBRVwiLFxuXHRcdGx0OiBcIjxcIixcblx0XHRndDogXCI+XCIsXG5cdFx0YW1wOiBcIiZcIixcblx0XHRhcG9zOiBcIidcIixcblx0XHRxdW90OiBgXCJgLFxuXHR9O1xuXG5cdHJldHVybiBpbnB1dC5yZXBsYWNlKC8mKFteO10rKTsvZywgKGVudGl0eTogc3RyaW5nLCBlbnRpdHlDb2RlOiBzdHJpbmcpID0+IHtcblx0XHRjb25zdCBtYXliZUVudGl0eSA9IE9iamVjdC5lbnRyaWVzKGh0bWxFbnRpdGllcykuZmluZCgoW2tleSwgX10pID0+IGtleSA9PT0gZW50aXR5Q29kZSk7XG5cdFx0aWYgKG1heWJlRW50aXR5KSByZXR1cm4gbWF5YmVFbnRpdHlbMV07XG5cblx0XHRsZXQgbWF0Y2g7XG5cdFx0aWYgKChtYXRjaCA9IGVudGl0eUNvZGUubWF0Y2goL14jeChbXFxkYS1mQS1GXSspJC8pKSkge1xuXHRcdFx0cmV0dXJuIFN0cmluZy5mcm9tQ2hhckNvZGUocGFyc2VJbnQobWF0Y2hbMV0sIDE2KSk7XG5cdFx0fVxuXHRcdGlmICgobWF0Y2ggPSBlbnRpdHlDb2RlLm1hdGNoKC9eIyhcXGQrKSQvKSkpIHtcblx0XHRcdHJldHVybiBTdHJpbmcuZnJvbUNoYXJDb2RlKH5+bWF0Y2hbMV0pO1xuXHRcdH1cblx0XHRyZXR1cm4gZW50aXR5O1xuXHR9KTtcbn1cblxuZnVuY3Rpb24gdHJpbUJsYW5rTGluZXMoaW5wdXQ6IHN0cmluZyk6IHN0cmluZyB7XG5cdGxldCByZXN1bHQgPSBpbnB1dC5yZXBsYWNlKC9cXHIvZywgXCJcIik7XG5cblx0d2hpbGUgKHJlc3VsdC5jaGFyQXQoMCkgPT09IFwiXFxyXCIgfHwgcmVzdWx0LmNoYXJBdCgwKSA9PT0gXCJcXG5cIikge1xuXHRcdHJlc3VsdCA9IHJlc3VsdC5zbGljZSgxKTtcblx0fVxuXHR3aGlsZSAocmVzdWx0LnNsaWNlKC0xKSA9PT0gXCJcXHJcIiB8fCByZXN1bHQuc2xpY2UoLTEpID09PSBcIlxcblwiKSB7XG5cdFx0cmVzdWx0ID0gcmVzdWx0LnNsaWNlKDAsIC0xKTtcblx0fVxuXG5cdHJldHVybiByZXN1bHQ7XG59XG5cbmZ1bmN0aW9uIGdldEFjY2VwdGVkTGljZW5zZXMoKSB7XG5cdGNvbnN0IHRvbWxDb250ZW50ID0gZnMucmVhZEZpbGVTeW5jKHBhdGgucmVzb2x2ZShfX2Rpcm5hbWUsIFwiLi4vYWJvdXQudG9tbFwiKSwgXCJ1dGY4XCIpO1xuXG5cdGNvbnN0IGxpY2Vuc2VzQmxvY2sgPSB0b21sQ29udGVudD8ubWF0Y2goL2FjY2VwdGVkXFxzKj1cXHMqXFxbKFteXFxdXSopXFxdLyk/LlsxXSB8fCBcIlwiO1xuXG5cdHJldHVybiBsaWNlbnNlc0Jsb2NrXG5cdFx0LnNwbGl0KFwiXFxuXCIpXG5cdFx0Lm1hcCgobGluZSkgPT4gbGluZS5yZXBsYWNlKC8jLiokLywgXCJcIikpIC8vIFJlbW92ZSBjb21tZW50c1xuXHRcdC5qb2luKFwiXFxuXCIpXG5cdFx0LnNwbGl0KFwiLFwiKVxuXHRcdC5tYXAoKGxpY2Vuc2UpID0+IGxpY2Vuc2UudHJpbSgpLnJlcGxhY2UoL1wiL2csIFwiXCIpKVxuXHRcdC5maWx0ZXIoKGxpY2Vuc2UpID0+IGxpY2Vuc2UubGVuZ3RoID4gMClcblx0XHQuam9pbihcIiBPUiBcIik7XG59XG4iXSwKICAibWFwcGluZ3MiOiAiO0FBRUEsU0FBUyxpQkFBaUI7QUFDMUIsT0FBTyxRQUFRO0FBQ2YsT0FBTyxVQUFVO0FBRWpCLFNBQVMsY0FBYztBQUN2QixPQUFPLHlCQUE4QztBQUNyRCxTQUFTLHdCQUF3QjtBQUNqQyxTQUFTLG9CQUFvQjtBQUM3QixTQUFTLDBCQUEwQiwwQkFBMEI7QUFWN0QsSUFBTSxtQ0FBbUM7QUFZekMsSUFBTSxpQkFBaUIsS0FBSyxRQUFRLGdDQUFTO0FBRzdDLElBQU8sc0JBQVEsYUFBYTtBQUFBLEVBQzNCLFNBQVM7QUFBQSxJQUNSLE9BQU87QUFBQSxNQUNOLFlBQVksQ0FBQyxpQkFBaUIsQ0FBQztBQUFBLE1BQy9CLE9BQU8sU0FBUyxnQkFBZ0I7QUFDL0IsY0FBTSxhQUFhO0FBQUEsVUFDbEI7QUFBQTtBQUFBLFVBQ0E7QUFBQTtBQUFBLFVBQ0E7QUFBQTtBQUFBLFVBQ0E7QUFBQTtBQUFBLFVBQ0E7QUFBQTtBQUFBLFFBQ0Q7QUFDQSxZQUFJLFdBQVcsU0FBUyxRQUFRLElBQUksRUFBRztBQUV2Qyx5QkFBaUIsT0FBTztBQUFBLE1BQ3pCO0FBQUEsSUFDRCxDQUFDO0FBQUEsSUFDRDtBQUFBO0FBQUEsTUFFQyxDQUFDLEVBQUUsT0FBTyxzQkFBc0IsUUFBUSxlQUFlLENBQUM7QUFBQTtBQUFBLE1BRXhELEVBQUUsV0FBVyxFQUFFLGFBQWEsbUJBQW1CLEVBQUU7QUFBQSxJQUNsRDtBQUFBLEVBQ0Q7QUFBQSxFQUNBLFNBQVM7QUFBQSxJQUNSLE9BQU87QUFBQSxNQUNOLEVBQUUsTUFBTSxpQ0FBaUMsYUFBYSxLQUFLLFFBQVEsZ0JBQWdCLFFBQVEsRUFBRTtBQUFBLE1BQzdGLEVBQUUsTUFBTSxzQkFBc0IsYUFBYSxlQUFlO0FBQUEsTUFDMUQsRUFBRSxNQUFNLHVCQUF1QixhQUFhLEtBQUssUUFBUSxnQkFBZ0IsUUFBUSxFQUFFO0FBQUEsTUFDbkYsRUFBRSxNQUFNLHVCQUF1QixhQUFhLEtBQUssUUFBUSxnQkFBZ0IsUUFBUSxFQUFFO0FBQUEsTUFDbkYsRUFBRSxNQUFNLGFBQWEsYUFBYSxLQUFLLFFBQVEsZ0JBQWdCLEtBQUssRUFBRTtBQUFBLElBQ3ZFO0FBQUEsRUFDRDtBQUFBLEVBQ0EsUUFBUTtBQUFBLElBQ1AsTUFBTTtBQUFBLElBQ04sTUFBTTtBQUFBLEVBQ1A7QUFBQSxFQUNBLE9BQU87QUFBQSxJQUNOLGVBQWU7QUFBQSxNQUNkLFNBQVM7QUFBQSxRQUNSLG9CQUFvQjtBQUFBLFVBQ25CLFlBQVk7QUFBQSxZQUNYLGdCQUFnQjtBQUFBLFlBQ2hCLGtCQUFrQjtBQUFBLFlBQ2xCLE9BQU87QUFBQSxjQUNOLE1BQU0sSUFBSSxvQkFBb0IsQ0FBQztBQUFBLGNBQy9CLGtCQUFrQjtBQUFBLGNBQ2xCLGlCQUFpQjtBQUFBLFlBQ2xCO0FBQUEsWUFDQSxRQUFRO0FBQUEsY0FDUCxNQUFNLEtBQUssUUFBUSxrQ0FBVyxpQ0FBaUM7QUFBQSxjQUMvRCxVQUFVO0FBQUEsWUFDWDtBQUFBLFVBQ0Q7QUFBQSxRQUNELENBQUM7QUFBQSxNQUNGO0FBQUEsSUFDRDtBQUFBLEVBQ0Q7QUFDRCxDQUFDO0FBZ0JELFNBQVMseUJBQXlCLFlBQWtDO0FBRW5FLFFBQU0sZUFBZSxxQkFBcUI7QUFDMUMsUUFBTSxxQkFBcUIsMkJBQTJCO0FBR3RELE1BQUksYUFBYSxXQUFXLEdBQUc7QUFFOUIsWUFBUSxNQUFNLGlGQUFpRjtBQUMvRixZQUFRLE1BQU0saUZBQWlGO0FBQy9GLFlBQVEsTUFBTSxpRUFBaUU7QUFFL0UsWUFBUSxLQUFLLENBQUM7QUFBQSxFQUNmO0FBQ0EsTUFBSSxXQUFXLFdBQVcsR0FBRztBQUM1QixZQUFRLE1BQU0sMkZBQTJGO0FBQ3pHLFlBQVEsTUFBTSxpRUFBaUU7QUFFL0UsWUFBUSxLQUFLLENBQUM7QUFBQSxFQUNmO0FBRUEsTUFBSSxXQUFXLGFBQWEsT0FBTyxrQkFBa0I7QUFHckQsTUFBSSxxQkFBeUM7QUFDN0MsTUFBSSxxQkFBeUM7QUFDN0MsV0FBUyxRQUFRLENBQUMsU0FBUyxpQkFBaUI7QUFDM0MsWUFBUSxTQUFTLFFBQVEsQ0FBQyxLQUFLLGFBQWE7QUFDM0MsVUFBSSxJQUFJLFNBQVMsYUFBYTtBQUM3Qiw2QkFBcUI7QUFDckIsNkJBQXFCO0FBQUEsTUFDdEI7QUFBQSxJQUNELENBQUM7QUFBQSxFQUNGLENBQUM7QUFDRCxNQUFJLHVCQUF1QixVQUFhLHVCQUF1QixRQUFXO0FBQ3pFLFVBQU0sVUFBVSxTQUFTLGtCQUFrQjtBQUMzQyxVQUFNLE1BQU0sUUFBUSxTQUFTLGtCQUFrQjtBQUUvQyxZQUFRLFdBQVcsUUFBUSxTQUFTLE9BQU8sQ0FBQ0EsU0FBUUEsS0FBSSxTQUFTLFdBQVc7QUFDNUUsVUFBTSxhQUFhLEdBQUcsYUFBYSxLQUFLLFFBQVEsa0NBQVcsK0JBQStCLEdBQUcsTUFBTTtBQUVuRyxhQUFTLEtBQUs7QUFBQSxNQUNiLGFBQWEsUUFBUTtBQUFBLE1BQ3JCLGFBQWEsUUFBUTtBQUFBLE1BQ3JCO0FBQUEsTUFDQSxVQUFVLENBQUMsR0FBRztBQUFBLElBQ2YsQ0FBQztBQUFBLEVBQ0Y7QUFHQSxhQUFXLFFBQVEsQ0FBQyxjQUFjO0FBQ2pDLFVBQU0sT0FBTyxVQUFVLFFBQVE7QUFDL0IsVUFBTSxVQUFVLFVBQVUsV0FBVztBQUNyQyxVQUFNLFNBQVMsVUFBVSxRQUFRLEtBQUssS0FBSztBQUMzQyxVQUFNLGNBQWMsVUFBVSxXQUFXO0FBQ3pDLFVBQU0sY0FBYyxlQUFlLFVBQVUsZUFBZSxFQUFFO0FBQzlELFVBQU0sYUFBYSxlQUFlLFVBQVUsY0FBYyxFQUFFO0FBRTVELFFBQUksYUFBYSxVQUFVLGNBQWM7QUFDekMsUUFBSSxjQUFjLE9BQU8sZUFBZSxTQUFVLGNBQWEsV0FBVztBQUUxRSxVQUFNLGlCQUFpQixTQUFTO0FBQUEsTUFDL0IsQ0FBQyxZQUFZLFFBQVEsZ0JBQWdCLGVBQWUsZUFBZSxRQUFRLGVBQWUsRUFBRSxNQUFNLGVBQWUsZUFBZSxRQUFRLGNBQWMsRUFBRSxNQUFNO0FBQUEsSUFDL0o7QUFFQSxVQUFNLE1BQW1CLEVBQUUsTUFBTSxTQUFTLFFBQVEsV0FBVztBQUM3RCxRQUFJLGVBQWdCLGdCQUFlLFNBQVMsS0FBSyxHQUFHO0FBQUEsUUFDL0MsVUFBUyxLQUFLLEVBQUUsYUFBYSxhQUFhLFlBQVksVUFBVSxDQUFDLEdBQUcsRUFBRSxDQUFDO0FBQUEsRUFDN0UsQ0FBQztBQUdELFdBQVMsUUFBUSxDQUFDLFNBQVMsVUFBVTtBQUNwQyxRQUFJLFFBQVEsWUFBWTtBQUN2QixlQUFTLEtBQUssRUFBRSxlQUFlO0FBQy9CLGVBQVMsS0FBSyxFQUFFLGVBQWU7QUFDL0IsZUFBUyxLQUFLLEVBQUUsZUFBZTtBQUMvQixlQUFTLEtBQUssRUFBRSxlQUFlO0FBQy9CLGVBQVMsS0FBSyxFQUFFLGVBQWU7QUFDL0IsZUFBUyxLQUFLLEVBQUUsZUFBZTtBQUMvQixlQUFTLEtBQUssRUFBRSxlQUFlLEdBQUcsUUFBUSxVQUFVO0FBQUE7QUFDcEQsZUFBUyxLQUFLLEVBQUUsYUFBYTtBQUFBLElBQzlCO0FBQUEsRUFDRCxDQUFDO0FBR0QsUUFBTSwrQkFBK0IsU0FBUyxJQUFJLENBQUMsWUFBWSxRQUFRLFlBQVksUUFBUSxZQUFZLEdBQUcsRUFBRSxLQUFLLENBQUM7QUFDbEgsV0FBUyxRQUFRLENBQUMsZ0JBQWdCLHdCQUF3QjtBQUN6RCxhQUFTLE1BQU0sR0FBRyxtQkFBbUIsRUFBRSxRQUFRLENBQUMsbUJBQW1CLDJCQUEyQjtBQUM3RixVQUFJLDZCQUE2QixtQkFBbUIsTUFBTSw2QkFBNkIsc0JBQXNCLEdBQUc7QUFDL0csdUJBQWUsU0FBUyxLQUFLLEdBQUcsa0JBQWtCLFFBQVE7QUFDMUQsMEJBQWtCLFdBQVcsQ0FBQztBQUFBLE1BRS9CO0FBQUEsSUFDRCxDQUFDO0FBQUEsRUFDRixDQUFDO0FBR0QsYUFBVyxTQUFTLE9BQU8sQ0FBQyxZQUFZO0FBQ3ZDLFlBQVEsV0FBVyxRQUFRLFNBQVM7QUFBQSxNQUNuQyxDQUFDLGdCQUNBLEVBQUUsWUFBWSxjQUFjLFlBQVksV0FBVyxZQUFZLEVBQUUsU0FBUyxxQ0FBcUMsWUFBWSxDQUFDLE1BQzVILEVBQ0MsWUFBWSxVQUNaLFlBQVksT0FBTyxZQUFZLEVBQUUsU0FBUyxxQkFBcUI7QUFBQSxNQUUvRCxDQUFDLFlBQVksT0FBTyxZQUFZLEVBQUUsU0FBUyxHQUFHO0FBQUEsSUFFakQ7QUFDQSxXQUFPLFFBQVEsU0FBUyxTQUFTO0FBQUEsRUFDbEMsQ0FBQztBQUdELFdBQVMsS0FBSyxDQUFDLEdBQUcsTUFBTSxFQUFFLFlBQVksY0FBYyxFQUFFLFdBQVcsQ0FBQztBQUNsRSxXQUFTLEtBQUssQ0FBQyxHQUFHLE1BQU0sRUFBRSxZQUFZLGNBQWMsRUFBRSxXQUFXLENBQUM7QUFDbEUsV0FBUyxLQUFLLENBQUMsR0FBRyxNQUFNLEVBQUUsU0FBUyxTQUFTLEVBQUUsU0FBUyxNQUFNO0FBRTdELFdBQVMsUUFBUSxDQUFDLFlBQVk7QUFDN0IsWUFBUSxTQUFTLEtBQUssQ0FBQyxHQUFHLE1BQU0sRUFBRSxLQUFLLGNBQWMsRUFBRSxJQUFJLENBQUM7QUFBQSxFQUM3RCxDQUFDO0FBR0QsTUFBSSx5QkFBeUI7QUFDN0IsNEJBQTBCO0FBQzFCLDRCQUEwQjtBQUMxQiw0QkFBMEI7QUFDMUIsNEJBQTBCO0FBQzFCLDRCQUEwQjtBQUcxQixXQUFTLFFBQVEsQ0FBQyxZQUFZO0FBQzdCLFFBQUksMEJBQTBCLFFBQVEsU0FBUyxJQUFJLENBQUMsZ0JBQWdCO0FBQ25FLFlBQU0sRUFBRSxNQUFNLFNBQVMsUUFBUSxXQUFXLElBQUk7QUFHOUMsVUFBSSxPQUFPO0FBQ1gsVUFBSSxLQUFLLFdBQVcsTUFBTSxFQUFHLFFBQU8sS0FBSyxNQUFNLE9BQU8sTUFBTTtBQUM1RCxVQUFJLEtBQUssV0FBVyxRQUFRLEVBQUcsUUFBTyxLQUFLLE1BQU0sU0FBUyxNQUFNO0FBQ2hFLFVBQUksS0FBSyxTQUFTLE1BQU0sRUFBRyxRQUFPLEtBQUssTUFBTSxHQUFHLENBQUMsT0FBTyxNQUFNO0FBQzlELFVBQUksS0FBSyxTQUFTLGNBQWMsRUFBRyxRQUFPLEtBQUssTUFBTSxHQUFHLENBQUMsZUFBZSxNQUFNO0FBRTlFLGFBQU8sR0FBRyxJQUFJLElBQUksT0FBTyxHQUFHLFNBQVMsTUFBTSxNQUFNLEtBQUssRUFBRSxHQUFHLE9BQU8sTUFBTSxJQUFJLEtBQUssRUFBRTtBQUFBLElBQ3BGLENBQUM7QUFDRCxVQUFNLFFBQVEsd0JBQXdCLFdBQVc7QUFDakQsVUFBTSxjQUFjLFFBQVEsWUFBWSxZQUFZLEVBQUUsU0FBUyxTQUFTO0FBQ3hFLFVBQU0sU0FBUyxjQUFjLFFBQVEsTUFBTSxFQUFFLGdCQUFnQixRQUFRLFFBQVEsSUFBSSxvQ0FBb0MsUUFBUSxXQUFXLEdBQUcsY0FBYyxLQUFLLFVBQVU7QUFDeEssVUFBTSxxQkFBcUIsS0FBSyxJQUFJLE9BQU8sUUFBUSxHQUFHLHdCQUF3QixJQUFJLENBQUMsU0FBUyxLQUFLLE1BQU0sQ0FBQztBQUN4Ryw4QkFBMEIsd0JBQXdCLElBQUksQ0FBQyxTQUFTLFVBQUssSUFBSSxHQUFHLElBQUksT0FBTyxxQkFBcUIsS0FBSyxNQUFNLENBQUMsU0FBSTtBQUU1SCw4QkFBMEI7QUFDMUIsOEJBQTBCLElBQUksSUFBSSxPQUFPLHFCQUFxQixDQUFDLENBQUM7QUFBQTtBQUNoRSw4QkFBMEIsVUFBSyxJQUFJLE9BQU8sa0JBQWtCLENBQUM7QUFBQTtBQUM3RCw4QkFBMEIsVUFBSyxNQUFNLEdBQUcsSUFBSSxPQUFPLHFCQUFxQixPQUFPLE1BQU0sQ0FBQztBQUFBO0FBQ3RGLDhCQUEwQixTQUFJLElBQUksT0FBTyxxQkFBcUIsQ0FBQyxDQUFDO0FBQUE7QUFDaEUsOEJBQTBCLEdBQUcsd0JBQXdCLEtBQUssSUFBSSxDQUFDO0FBQUE7QUFDL0QsOEJBQTBCLElBQUksU0FBSSxPQUFPLHFCQUFxQixDQUFDLENBQUM7QUFBQTtBQUNoRSw4QkFBMEIsR0FBRyxRQUFRLFdBQVc7QUFBQTtBQUFBLEVBQ2pELENBQUM7QUFFRCw0QkFBMEI7QUFDMUIsU0FBTztBQUNSO0FBR0EsU0FBUyw2QkFBNEM7QUFDcEQsUUFBTSxzQkFBc0I7QUFBQSxJQUMzQjtBQUFBLE1BQ0MsYUFBYTtBQUFBLE1BQ2IsaUJBQWlCO0FBQUEsTUFDakIsY0FBYztBQUFBLElBQ2Y7QUFBQSxJQUNBO0FBQUEsTUFDQyxhQUFhO0FBQUEsTUFDYixpQkFBaUI7QUFBQSxNQUNqQixjQUFjO0FBQUEsSUFDZjtBQUFBLEVBQ0Q7QUFFQSxTQUFPLG9CQUFvQixJQUFJLENBQUMsRUFBRSxhQUFhLGlCQUFpQixhQUFhLE1BQU07QUFDbEYsVUFBTSxjQUFlLEdBQUcsV0FBVyxlQUFlLEtBQUssR0FBRyxhQUFhLGlCQUFpQixNQUFNLEtBQU07QUFFcEcsVUFBTSxlQUFnQixHQUFHLFdBQVcsWUFBWSxLQUFLLEtBQUssTUFBTSxHQUFHLGFBQWEsY0FBYyxNQUFNLENBQUMsS0FBTSxDQUFDO0FBQzVHLFVBQU0sT0FBTyxhQUFhLFFBQVE7QUFDbEMsVUFBTSxVQUFVLGFBQWEsV0FBVztBQUN4QyxVQUFNLFNBQVMsYUFBYSxPQUFPLFFBQVEsYUFBYSxVQUFVO0FBQ2xFLFVBQU0sYUFBYSxhQUFhLFlBQVksT0FBTztBQUVuRCxXQUFPO0FBQUEsTUFDTjtBQUFBLE1BQ0EsYUFBYSxlQUFlLFdBQVc7QUFBQSxNQUN2QyxVQUFVLENBQUMsRUFBRSxNQUFNLFNBQVMsUUFBUSxXQUFXLENBQUM7QUFBQSxJQUNqRDtBQUFBLEVBQ0QsQ0FBQztBQUNGO0FBRUEsU0FBUyx1QkFBc0M7QUFFOUMsVUFBUSxLQUFLLG9EQUFvRDtBQUVqRSxNQUFJO0FBR0gsVUFBTSxFQUFFLFFBQVEsUUFBUSxPQUFPLElBQUksVUFBVSxTQUFTLENBQUMsU0FBUyxZQUFZLFdBQVcsR0FBRztBQUFBLE1BQ3pGLEtBQUssS0FBSyxLQUFLLGtDQUFXLElBQUk7QUFBQSxNQUM5QixVQUFVO0FBQUEsTUFDVixPQUFPO0FBQUEsTUFDUCxhQUFhO0FBQUE7QUFBQSxJQUNkLENBQUM7QUFHRCxRQUFJLFdBQVcsR0FBRztBQUVqQixVQUFJLFdBQVcsS0FBSztBQUNuQixnQkFBUSxNQUFNLHNCQUFzQixRQUFRLE1BQU07QUFBQSxNQUNuRDtBQUNBLGFBQU8sQ0FBQztBQUFBLElBQ1Q7QUFJQSxRQUFJLENBQUMsT0FBTyxLQUFLLEVBQUUsV0FBVywyQkFBMkIsR0FBRztBQUMzRCxjQUFRLE1BQU0sc0NBQXNDLE1BQU07QUFDMUQsYUFBTyxDQUFDO0FBQUEsSUFDVDtBQUtBLFVBQU0sZUFBZTtBQUNyQixVQUFNLGdCQUFnQixhQUFhLE1BQU07QUFHekMsVUFBTSxnQkFBZ0IsaUJBQWlCLENBQUMsR0FBRztBQUFBLE1BQzFDLENBQUMsaUJBQThCO0FBQUEsUUFDOUIsYUFBYSxXQUFXLFlBQVksV0FBVztBQUFBLFFBQy9DLGFBQWEsZUFBZSxXQUFXLFlBQVksV0FBVyxDQUFDO0FBQUEsUUFDL0QsVUFBVSxZQUFZLFNBQVM7QUFBQSxVQUM5QixDQUFDLGlCQUE4QjtBQUFBLFlBQzlCLE1BQU0sV0FBVyxZQUFZLElBQUk7QUFBQSxZQUNqQyxTQUFTLFdBQVcsWUFBWSxPQUFPO0FBQUEsWUFDdkMsUUFBUSxXQUFXLFlBQVksTUFBTSxFQUNuQyxRQUFRLGNBQWMsSUFBSSxFQUMxQixRQUFRLE1BQU0sRUFBRTtBQUFBLFlBQ2xCLFlBQVksV0FBVyxZQUFZLFVBQVU7QUFBQSxVQUM5QztBQUFBLFFBQ0Q7QUFBQSxNQUNEO0FBQUEsSUFDRDtBQUVBLFdBQU87QUFBQSxFQUNSLFNBQVMsR0FBRztBQUNYLFdBQU8sQ0FBQztBQUFBLEVBQ1Q7QUFDRDtBQUVBLFNBQVMsV0FBVyxPQUF1QjtBQUMxQyxNQUFJLENBQUMsTUFBTyxRQUFPO0FBRW5CLFFBQU0sZUFBZTtBQUFBLElBQ3BCLE1BQU07QUFBQSxJQUNOLE1BQU07QUFBQSxJQUNOLEtBQUs7QUFBQSxJQUNMLElBQUk7QUFBQSxJQUNKLElBQUk7QUFBQSxJQUNKLEtBQUs7QUFBQSxJQUNMLE1BQU07QUFBQSxJQUNOLE1BQU07QUFBQSxFQUNQO0FBRUEsU0FBTyxNQUFNLFFBQVEsY0FBYyxDQUFDLFFBQWdCLGVBQXVCO0FBQzFFLFVBQU0sY0FBYyxPQUFPLFFBQVEsWUFBWSxFQUFFLEtBQUssQ0FBQyxDQUFDLEtBQUssQ0FBQyxNQUFNLFFBQVEsVUFBVTtBQUN0RixRQUFJLFlBQWEsUUFBTyxZQUFZLENBQUM7QUFFckMsUUFBSTtBQUNKLFFBQUssUUFBUSxXQUFXLE1BQU0sbUJBQW1CLEdBQUk7QUFDcEQsYUFBTyxPQUFPLGFBQWEsU0FBUyxNQUFNLENBQUMsR0FBRyxFQUFFLENBQUM7QUFBQSxJQUNsRDtBQUNBLFFBQUssUUFBUSxXQUFXLE1BQU0sVUFBVSxHQUFJO0FBQzNDLGFBQU8sT0FBTyxhQUFhLENBQUMsQ0FBQyxNQUFNLENBQUMsQ0FBQztBQUFBLElBQ3RDO0FBQ0EsV0FBTztBQUFBLEVBQ1IsQ0FBQztBQUNGO0FBRUEsU0FBUyxlQUFlLE9BQXVCO0FBQzlDLE1BQUksU0FBUyxNQUFNLFFBQVEsT0FBTyxFQUFFO0FBRXBDLFNBQU8sT0FBTyxPQUFPLENBQUMsTUFBTSxRQUFRLE9BQU8sT0FBTyxDQUFDLE1BQU0sTUFBTTtBQUM5RCxhQUFTLE9BQU8sTUFBTSxDQUFDO0FBQUEsRUFDeEI7QUFDQSxTQUFPLE9BQU8sTUFBTSxFQUFFLE1BQU0sUUFBUSxPQUFPLE1BQU0sRUFBRSxNQUFNLE1BQU07QUFDOUQsYUFBUyxPQUFPLE1BQU0sR0FBRyxFQUFFO0FBQUEsRUFDNUI7QUFFQSxTQUFPO0FBQ1I7QUFFQSxTQUFTLHNCQUFzQjtBQUM5QixRQUFNLGNBQWMsR0FBRyxhQUFhLEtBQUssUUFBUSxrQ0FBVyxlQUFlLEdBQUcsTUFBTTtBQUVwRixRQUFNLGdCQUFnQixhQUFhLE1BQU0sNkJBQTZCLElBQUksQ0FBQyxLQUFLO0FBRWhGLFNBQU8sY0FDTCxNQUFNLElBQUksRUFDVixJQUFJLENBQUMsU0FBUyxLQUFLLFFBQVEsUUFBUSxFQUFFLENBQUMsRUFDdEMsS0FBSyxJQUFJLEVBQ1QsTUFBTSxHQUFHLEVBQ1QsSUFBSSxDQUFDLFlBQVksUUFBUSxLQUFLLEVBQUUsUUFBUSxNQUFNLEVBQUUsQ0FBQyxFQUNqRCxPQUFPLENBQUMsWUFBWSxRQUFRLFNBQVMsQ0FBQyxFQUN0QyxLQUFLLE1BQU07QUFDZDsiLAogICJuYW1lcyI6IFsicGtnIl0KfQo=
