import { getEmbedderName, getIndexPopulated, getSearchKey, serverEnv } from "@/lib/server-config";

export async function GET() {
  try {
    const [apiKey, embedder, chunkEmbedder, legiEmbedder, hasChunks, hasLegi] = await Promise.all([
      getSearchKey(),
      getEmbedderName(),
      getEmbedderName(serverEnv.chunkIndex),
      getEmbedderName(serverEnv.legiIndex),
      getIndexPopulated(serverEnv.chunkIndex),
      getIndexPopulated(serverEnv.legiIndex),
    ]);
    return Response.json({
      host: serverEnv.publicMeiliUrl,
      apiKey,
      index: serverEnv.index,
      chunkIndex: hasChunks ? serverEnv.chunkIndex : null,
      legiIndex: hasLegi ? serverEnv.legiIndex : null,
      embedder,
      chunkEmbedder,
      legiEmbedder,
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : "unknown error";
    return Response.json({ error: `Meilisearch is not ready: ${message}` }, { status: 503 });
  }
}
