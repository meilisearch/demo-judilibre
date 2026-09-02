"use client";

import { Meilisearch } from "meilisearch";
import type { SearchConfig } from "@/lib/types";

let cached: Promise<SearchConfig> | null = null;

/** Fetch the public Meilisearch host + search-only key from our server once. */
export function getSearchConfig(): Promise<SearchConfig> {
  if (!cached) {
    cached = fetch("/api/config")
      .then(async (r) => {
        if (!r.ok) throw new Error(`config: HTTP ${r.status}`);
        return (await r.json()) as SearchConfig;
      })
      .catch((e) => {
        cached = null;
        throw e;
      });
  }
  return cached;
}

let client: Meilisearch | null = null;

export async function getMeili(): Promise<{ client: Meilisearch; config: SearchConfig }> {
  const config = await getSearchConfig();
  client ??= new Meilisearch({ host: config.host, apiKey: config.apiKey });
  return { client, config };
}
