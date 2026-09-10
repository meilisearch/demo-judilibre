import { getEmbedderName, getIndexPopulated, getSearchKey, serverEnv } from "@/lib/server-config";

export async function GET() {
  try {
    const [apiKey, embedder, legiEmbedder, hasLegi] = await Promise.all([
      getSearchKey(),
      getEmbedderName(),
      getEmbedderName(serverEnv.legiIndex),
      getIndexPopulated(serverEnv.legiIndex),
    ]);
    return Response.json({
      host: serverEnv.publicMeiliUrl,
      apiKey,
      index: serverEnv.index,
      legiIndex: hasLegi ? serverEnv.legiIndex : null,
      embedder,
      legiEmbedder,
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : "unknown error";
    return Response.json({ error: `Meilisearch is not ready: ${message}` }, { status: 503 });
  }
}
