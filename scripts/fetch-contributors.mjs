import { existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const OUTPUT = resolve(__dirname, "../src/generated/contributors.json");

function writeContributors(contributors) {
  mkdirSync(dirname(OUTPUT), { recursive: true });
  writeFileSync(OUTPUT, JSON.stringify(contributors, null, 2) + "\n");
}

// Renown: 本项目基于 floral-notepaper 分叉(MIT)。
// 构建期不再自动拉取上游贡献者头像,避免把原作者信息当作本应用作者展示。
// 上游署名保留在 THIRD_PARTY_NOTICES.md 与 README 中。
// 如需启用,设置 RENOWN_UPSTREAM_REPO="owner/repo"。
const REPO = process.env.RENOWN_UPSTREAM_REPO || "";

async function fetchContributors() {
  if (!REPO) {
    return [];
  }

  const headers = { "User-Agent": "renown-build" };
  const token = process.env.GH_TOKEN || process.env.GITHUB_TOKEN;
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const res = await fetch(`https://api.github.com/repos/${REPO}/contributors?per_page=100`, {
    headers,
  });
  if (!res.ok) {
    throw new Error(`GitHub API responded ${res.status}: ${res.statusText}`);
  }

  const data = await res.json();
  return data
    .filter((u) => u.type !== "Bot")
    .map((u) => ({
      login: u.login,
      avatar_url: u.avatar_url,
      html_url: u.html_url,
    }));
}

try {
  const contributors = await fetchContributors();
  writeContributors(contributors);
  console.log(`[contributors] wrote ${contributors.length} contributors`);
} catch (err) {
  if (existsSync(OUTPUT)) {
    const cached = JSON.parse(readFileSync(OUTPUT, "utf-8"));
    console.warn(
      `[contributors] API failed (${err.message}), keeping cached ${cached.length} contributors`,
    );
  } else {
    writeContributors([]);
    console.warn(`[contributors] API failed (${err.message}), wrote empty fallback`);
  }
}
