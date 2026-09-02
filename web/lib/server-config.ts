import "server-only";

const index = process.env.MEILI_INDEX ?? "decisions";

export const serverEnv = {
  meiliUrl: process.env.MEILI_URL ?? "http://localhost:7700",
  publicMeiliUrl: process.env.NEXT_PUBLIC_MEILI_URL ?? process.env.MEILI_URL ?? "http://localhost:7700",
  /**
   * Admin key. Optional: with it the app discovers keys and settings by itself,
   * which is convenient locally. A deployment can run on the search and chat keys
   * alone, so no admin credential has to leave the machine.
   */
  masterKey: process.env.MEILI_MASTER_KEY ?? "",
  /** Search-only key handed to the browser. */
  searchKey: process.env.MEILI_SEARCH_KEY || process.env.NEXT_PUBLIC_MEILI_SEARCH_KEY || "",
  index,
  chunkIndex: process.env.MEILI_CHUNK_INDEX || `${index}_chunk`,
  /** Embedder name on both indexes; empty disables the hybrid-search toggle. */
  embedderName: process.env.MEILI_EMBEDDER ?? "voyage",
  chatWorkspace: process.env.CHAT_WORKSPACE ?? "judilibre",
  chatModel: process.env.CHAT_MODEL ?? "gpt-5.5",
  chatKey: process.env.MEILI_CHAT_KEY || process.env.MEILI_MASTER_KEY || "",
};

interface MeiliKey {
  key: string;
  actions: string[];
  indexes: string[];
}

/** A key is usable only if it can search every index the UI queries. */
function covers(key: MeiliKey, indexes: string[]): boolean {
  if (!key.actions.some((a) => a === "search" || a === "*")) return false;
  return indexes.every((i) => key.indexes.includes("*") || key.indexes.includes(i));
}

let searchKeyCache: { value: string; at: number } | null = null;

/**
 * Resolve a search-only API key to hand to the browser.
 *
 * `MEILI_SEARCH_KEY` wins when it can reach both the decision and passage
 * indexes. A key scoped to a single index would break the passage search, so
 * when an admin key is available we look for a broader search-only key instead.
 */
export async function getSearchKey(): Promise<string> {
  if (!serverEnv.masterKey) return serverEnv.searchKey;
  if (searchKeyCache && Date.now() - searchKeyCache.at < 10 * 60_000) return searchKeyCache.value;

  const needed = [serverEnv.index, serverEnv.chunkIndex];
  const res = await fetch(`${serverEnv.meiliUrl}/keys?limit=100`, {
    headers: { Authorization: `Bearer ${serverEnv.masterKey}` },
    cache: "no-store",
  });
  if (!res.ok) throw new Error(`Meilisearch /keys returned ${res.status}`);
  const keys = ((await res.json()) as { results: MeiliKey[] }).results;

  const configured = serverEnv.searchKey ? keys.find((k) => k.key === serverEnv.searchKey) : undefined;
  const chosen =
    (configured && covers(configured, needed) ? configured : undefined) ??
    keys.find((k) => k.actions.length === 1 && k.actions[0] === "search" && covers(k, needed)) ??
    keys.find((k) => covers(k, needed) && !k.actions.includes("*"));

  const value = chosen?.key ?? serverEnv.searchKey;
  if (!value) throw new Error(`No search key can reach ${needed.join(" and ")}`);
  searchKeyCache = { value, at: Date.now() };
  return value;
}

/** Authenticated fetch, preferring the admin key and falling back to the search key. */
export async function meiliFetch(path: string, init?: RequestInit): Promise<Response> {
  const key = serverEnv.masterKey || (await getSearchKey());
  return fetch(`${serverEnv.meiliUrl}${path}`, {
    ...init,
    headers: {
      ...(init?.headers ?? {}),
      ...(key ? { Authorization: `Bearer ${key}` } : {}),
    },
    cache: "no-store",
  });
}

/** Search one index with the search-only key. */
export async function meiliSearch<T>(
  indexUid: string,
  body: Record<string, unknown>,
): Promise<{ hits: T[]; estimatedTotalHits?: number } | null> {
  const key = (await getSearchKey()) || serverEnv.masterKey;
  const res = await fetch(`${serverEnv.meiliUrl}/indexes/${encodeURIComponent(indexUid)}/search`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(key ? { Authorization: `Bearer ${key}` } : {}),
    },
    body: JSON.stringify(body),
    cache: "no-store",
  });
  if (!res.ok) return null;
  return (await res.json()) as { hits: T[]; estimatedTotalHits?: number };
}

const embedderCache = new Map<string, { value: string | null; at: number }>();

/**
 * Name of the embedder configured on an index, or null when semantic search is
 * unavailable. Verified against the index when an admin key is present;
 * otherwise `MEILI_EMBEDDER` is taken at its word.
 */
export async function getEmbedderName(indexUid = serverEnv.index): Promise<string | null> {
  if (!serverEnv.masterKey) return serverEnv.embedderName || null;
  const cached = embedderCache.get(indexUid);
  if (cached && Date.now() - cached.at < 60_000) return cached.value;
  let value: string | null = null;
  try {
    const res = await meiliFetch(`/indexes/${indexUid}/settings/embedders`);
    if (res.ok) {
      const embedders = (await res.json()) as Record<string, unknown> | null;
      value = embedders ? (Object.keys(embedders)[0] ?? null) : null;
    }
  } catch {
    value = null;
  }
  embedderCache.set(indexUid, { value, at: Date.now() });
  return value;
}

let chunksCache: { value: boolean; at: number } | null = null;

/** Whether the passage index exists and holds documents (search key is enough). */
export async function getChunkIndexAvailable(): Promise<boolean> {
  if (chunksCache && Date.now() - chunksCache.at < 60_000) return chunksCache.value;
  const result = await meiliSearch<unknown>(serverEnv.chunkIndex, { q: "", limit: 0 }).catch(() => null);
  const value = Boolean(result) && (result?.estimatedTotalHits ?? 0) > 0;
  chunksCache = { value, at: Date.now() };
  return value;
}
