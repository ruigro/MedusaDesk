#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const source = fs.readFileSync("docs/assets/releases.js", "utf8");
const assets = [
  { name: "rustdesk-1.4.7-armv7-sciter.deb" },
  { name: "rustdesk-1.4.7-x86_64-sciter.deb" },
];

function loadSelector(userAgent, platform, architecture) {
  const context = {
    window: {},
    navigator: {
      userAgent,
      platform,
      userAgentData: architecture ? { platform, architecture } : undefined,
    },
    document: {},
    Intl,
  };
  vm.createContext(context);
  vm.runInContext(
    source.replace(/loadRelease\(\);\s*$/, "") +
      "\nthis.releaseSelector = { detectPlatform, pickPrimaryAsset };",
    context,
  );
  return context.releaseSelector;
}

const x64 = loadSelector("Mozilla/5.0 (X11; Linux x86_64)", "Linux x86_64");
assert.equal(
  x64.pickPrimaryAsset(assets, x64.detectPlatform()).name,
  "rustdesk-1.4.7-x86_64-sciter.deb",
);

const arm = loadSelector("Mozilla/5.0 (X11; Linux armv7l)", "Linux armv7l", "arm");
assert.equal(
  arm.pickPrimaryAsset(assets, arm.detectPlatform()).name,
  "rustdesk-1.4.7-armv7-sciter.deb",
);

console.log("Linux release download selection passed");
