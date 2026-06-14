const fallbackRepo = "FarIsland-Corporation/MedusaDesk";

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

function pickPrimaryAsset(assets) {
  const preferred = [".exe", ".msi", ".zip"];
  return (
    preferred
      .map((ext) => assets.find((asset) => asset.name.toLowerCase().endsWith(ext)))
      .find(Boolean) || assets[0]
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

  const tag = release.tag_name || release.name || "Latest";
  releaseTitle.textContent = release.name || tag;
  releaseDate.textContent = `Published ${formatDate(release.published_at)} from ${repo}`;
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

  const primary = pickPrimaryAsset(assets);
  primaryDownload.href = primary.browser_download_url;
  primaryDownload.textContent = `Download ${primary.name}`;
  primaryDownload.classList.remove("disabled");
  primaryDownload.removeAttribute("aria-disabled");

  downloadList.innerHTML = assets
    .map((asset) => {
      const name = escapeHtml(asset.name);
      return `
        <div class="download-row">
          <div>
            <p class="asset-name">${name}</p>
            <p class="asset-meta">${formatBytes(asset.size)} · ${asset.download_count || 0} downloads</p>
          </div>
          <a class="download-action" href="${asset.browser_download_url}">Download</a>
        </div>
      `;
    })
    .join("");

  renderNotes(release.body || "");
}

function renderError(repo) {
  document.getElementById("release-title").textContent = "No release found";
  document.getElementById("release-date").textContent =
    `Create a GitHub release with assets in ${repo}, then this page will update automatically.`;
  document.getElementById("all-releases-link").href =
    `https://github.com/${repo}/releases`;
  document.getElementById("download-list").innerHTML =
    '<p class="muted">No downloadable release assets are available yet.</p>';
}

async function loadRelease() {
  const repo = inferRepo();
  try {
    const response = await fetch(`https://api.github.com/repos/${repo}/releases/latest`, {
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
