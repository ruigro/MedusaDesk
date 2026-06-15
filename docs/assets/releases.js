const fallbackRepo = "ruigro/MedusaDesk";
const releaseTag = "v0.1.2";

function inferRepo() {
  if (window.MEDUSA_RELEASE_REPO) {
    return window.MEDUSA_RELEASE_REPO;
  }

  const host = window.location.hostname;
  const pathRepo = window.location.pathname.split("/").filter(Boolean)[0];
  if (host.endsWith(".github.io")) {
    const owner = host.replace(".github.io", "");
    if (owner && pathRepo) {
      return `${owner}/${pathRepo}`;
    }
  }

  return fallbackRepo;
}

function detectPlatform() {
  const ua = navigator.userAgent.toLowerCase();
  const platform = (navigator.userAgentData?.platform || navigator.platform || "").toLowerCase();
  const arch = navigator.userAgentData?.architecture?.toLowerCase() || ua;

  if (ua.includes("windows") || platform.includes("win")) {
    return { key: "windows", label: "Windows" };
  }
  if (ua.includes("mac os") || platform.includes("mac")) {
    const isArm = arch.includes("arm") || arch.includes("aarch64");
    return { key: "macos", label: isArm ? "macOS Apple Silicon" : "macOS" };
  }
  if (ua.includes("linux") || platform.includes("linux")) {
    return { key: "linux", label: "Linux" };
  }
  return { key: "unknown", label: "your OS" };
}

function assetPlatform(asset) {
  const name = asset.name.toLowerCase();
  if (name.includes("windows") || name.includes("win") || name.endsWith(".exe") || name.endsWith(".msi")) {
    return "windows";
  }
  if (name.includes("macos") || name.includes("darwin") || name.endsWith(".dmg")) {
    return "macos";
  }
  if (name.includes("linux") || name.endsWith(".appimage") || name.endsWith(".deb") || name.endsWith(".rpm")) {
    return "linux";
  }
  return "unknown";
}

function formatBytes(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "unknown size";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function formatDate(value) {
  if (!value) return "";
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(new Date(value));
}

function pickPrimaryAsset(assets, platform) {
  const platformAssets = assets.filter((asset) => assetPlatform(asset) === platform.key);
  const preferred = platform.key === "macos"
    ? ["aarch64", "arm64", ".dmg", "x86_64"]
    : platform.key === "windows"
      ? [".msi", ".exe", ".zip"]
      : [".appimage", ".deb", ".rpm", ".zip"];
  const candidates = platformAssets.length ? platformAssets : assets;

  return (
    preferred
      .map((token) => candidates.find((asset) => asset.name.toLowerCase().includes(token)))
      .find(Boolean) || candidates[0]
  );
}

function escapeHtml(value) {
  return value.replace(/[&<>"']/g, (char) => {
    return {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#039;",
    }[char];
  });
}

function renderNotes(body) {
  const notes = document.getElementById("release-notes");
  if (!body) {
    notes.innerHTML = '<p class="muted">No release notes were added.</p>';
    return;
  }

  const safe = escapeHtml(body)
    .replace(/^### (.*)$/gm, "<h3>$1</h3>")
    .replace(/^## (.*)$/gm, "<h2>$1</h2>")
    .replace(/^# (.*)$/gm, "<h2>$1</h2>")
    .replace(/\n/g, "<br>");
  notes.innerHTML = safe;
}

function renderRelease(repo, release) {
  const releaseTitle = document.getElementById("release-title");
  const releaseDate = document.getElementById("release-date");
  const primaryDownload = document.getElementById("primary-download");
  const allReleases = document.getElementById("all-releases-link");
  const downloadList = document.getElementById("download-list");
  const platform = detectPlatform();

  const tag = release.tag_name || release.name || releaseTag;
  releaseTitle.textContent = release.name || tag;
  releaseDate.textContent = `Detected ${platform.label}. Published ${formatDate(release.published_at)} from ${repo}`;
  allReleases.href = `https://github.com/${repo}/releases`;

  const assets = release.assets || [];
  if (!assets.length) {
    primaryDownload.classList.add("disabled");
    primaryDownload.setAttribute("aria-disabled", "true");
    downloadList.innerHTML =
      '<p class="muted">This release does not have downloadable assets yet.</p>';
    renderNotes(release.body || "");
    return;
  }

  const primary = pickPrimaryAsset(assets, platform);
  if (primary && assetPlatform(primary) === platform.key) {
    primaryDownload.href = primary.browser_download_url;
    primaryDownload.textContent = `Download for ${platform.label}`;
  } else {
    primaryDownload.href = `https://github.com/${repo}/releases/tag/${tag}`;
    primaryDownload.textContent = `${platform.label} build coming soon`;
  }
  primaryDownload.classList.remove("disabled");
  primaryDownload.removeAttribute("aria-disabled");

  downloadList.innerHTML = assets
    .map((asset) => {
      const name = escapeHtml(asset.name);
      const isDetected = assetPlatform(asset) === platform.key;
      return `
        <div class="download-row${isDetected ? " recommended" : ""}">
          <div>
            <p class="asset-name">${name}</p>
            <p class="asset-meta">${formatBytes(asset.size)} - ${asset.download_count || 0} downloads${isDetected ? " - recommended for this device" : ""}</p>
          </div>
          <a class="download-action" href="${asset.browser_download_url}">Download</a>
        </div>
      `;
    })
    .join("");

  renderNotes(release.body || "");
}

function renderError(repo) {
  const releaseUrl = `https://github.com/${repo}/releases/tag/${releaseTag}`;
  document.getElementById("release-title").textContent = "Download Medusa Desk";
  document.getElementById("release-date").textContent =
    `Release data could not be loaded. Open the GitHub release directly.`;
  document.getElementById("primary-download").href = releaseUrl;
  document.getElementById("primary-download").textContent = "Open downloads";
  document.getElementById("primary-download").classList.remove("disabled");
  document.getElementById("primary-download").removeAttribute("aria-disabled");
  document.getElementById("all-releases-link").href = `https://github.com/${repo}/releases`;
  document.getElementById("download-list").innerHTML =
    `<p class="muted"><a href="${releaseUrl}">Open the release page</a> to download Medusa Desk.</p>`;
}

async function loadRelease() {
  const repo = inferRepo();
  try {
    const response = await fetch(`https://api.github.com/repos/${repo}/releases/tags/${releaseTag}`, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!response.ok) throw new Error(`GitHub returned ${response.status}`);
    const release = await response.json();
    renderRelease(repo, release);
  } catch (error) {
    renderError(repo);
  }
}

loadRelease();
