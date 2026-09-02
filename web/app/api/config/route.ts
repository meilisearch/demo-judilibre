import { getChunkIndexAvailable, getEmbedderName, getSearchKey, serverEnv } from "@/lib/server-config";

export async function GET() {
  try {
    const [apiKey, embedder, chunkEmbedder, hasChunks] = await Promise.all([
      getSearchKey(),
      getEmbedderName(),
      getEmbedderName(serverEnv.chunkIndex),
      getChunkIndexAvailable(),
    ]);
    return Response.json({
      host: serverEnv.publicMeiliUrl,
      apiKey,
      index: serverEnv.index,
      chunkIndex: hasChunks ? serverEnv.chunkIndex : null,
      embedder,
      chunkEmbedder,
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : "unknown error";
    return Response.json({ error: `Meilisearch is not ready: ${message}` }, { status: 503 });
  }
}
