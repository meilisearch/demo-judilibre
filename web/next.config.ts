import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { NextConfig } from "next";

/**
 * The whole demo shares one `.env` at the repository root (used by Docker Compose
 * and the Rust indexer). Load it here so `pnpm dev` inside `web/` sees the same
 * values; variables already set in the environment win.
 */
function loadRootEnv() {
  const file = resolve(process.cwd(), "..", ".env");
  if (!existsSync(file)) return;
  for (const raw of readFileSync(file, "utf8").split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq === -1) continue;
    const key = line.slice(0, eq).trim();
    const value = line.slice(eq + 1).trim().replace(/^(['"])(.*)\1$/, "$2");
    if (key && process.env[key] === undefined) process.env[key] = value;
  }
}
loadRootEnv();

const nextConfig: NextConfig = {};

export default nextConfig;
