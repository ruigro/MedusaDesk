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

const linuxArm64Assets = [
  ...assets,
  { name: "MedusaDesk-v0.1.8-linux-arm64.deb" },
];
const linuxArm64 = loadSelector("Mozilla/5.0 (X11; Linux aarch64)", "Linux aarch64", "arm");
assert.equal(
  linuxArm64.pickPrimaryAsset(linuxArm64Assets, linuxArm64.detectPlatform()).name,
  "MedusaDesk-v0.1.8-linux-arm64.deb",
);
assert.equal(
  x64.pickPrimaryAsset(linuxArm64Assets, x64.detectPlatform()).name,
  "rustdesk-1.4.7-x86_64-sciter.deb",
  "an x64 Linux PC must never be offered the ARM64 package",
);

const windowsAssets = [
  { name: "MedusaDesk-v0.1.8-windows-arm64.exe" },
  { name: "MedusaDesk-v0.1.8-windows-x64.exe" },
];
const windowsUserAgent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64)";

const windowsX64 = loadSelector(windowsUserAgent, "Win32", "x86");
assert.equal(
  windowsX64.pickPrimaryAsset(windowsAssets, windowsX64.detectPlatform()).name,
  "MedusaDesk-v0.1.8-windows-x64.exe",
  "an x64 Windows PC must never be offered the ARM64 installer",
);

const windowsArm = loadSelector(windowsUserAgent, "Win32");
assert.equal(
  windowsArm.pickPrimaryAsset(windowsAssets, windowsArm.detectPlatform("arm")).name,
  "MedusaDesk-v0.1.8-windows-arm64.exe",
  "the architecture client hint must select the ARM64 installer",
);

assert.equal(
  windowsArm.pickPrimaryAsset([windowsAssets[1]], windowsArm.detectPlatform("arm")),
  undefined,
  "Windows ARM64 must not fall back to the x64 installer as its own build",
);

console.log("Linux and Windows release download selection passed");
