# Judilibre × Meilisearch demo

Monorepo with three parts sharing one root `.env` (see `.env.example`):

- `indexer/` — Rust CLI (`cargo run -- setup|index`). Exports decisions from the Judilibre API (PISTE), extracts attached PDFs with `pdf-inspector`, chunks everything, and pushes to Meilisearch. Pure logic lives in `src/transform.rs`, `src/chunk.rs` and `src/attachments.rs` and is unit-tested; run `cargo test` and `cargo clippy` before finishing.
- `web/` — Next.js 16 App Router (TypeScript strict, Tailwind v4, shadcn/ui on Base UI, TanStack Query, Zustand). `next.config.ts` loads the root `.env`. Run `pnpm exec tsc --noEmit && pnpm lint` before finishing.
- `docs/` — Mintlify (`mint.json`, `openapi.yaml` for the web API).

## Three indexes

- `judilibre`: one document per decision (Judilibre API). Fields in `indexer/src/transform.rs`, mirrored in `web/lib/types.ts`.
- `judilibre_chunk`: ~2 000-character passages of each decision and of each attached PDF, with `distinctAttribute: decision_id`. Fields in `indexer/src/chunk.rs`, mirrored as `ChunkHit`.
- `legi`: in-force articles of the French codes, parsed from the LEGI bulk archive (DILA). Fields in `indexer/src/legi.rs`.

Keep the Rust structs and the TypeScript types in sync. All three carry the same Voyage AI embedder (`voyage`, model `voyage-law-2`) via Meilisearch's `rest` source, with a per-shape `documentTemplate` (`EmbedderKind`).

## Cross-linking decisions and articles

`indexer/src/refs.rs` reduces both corpora to one key, `code-du-travail:L1152-1`:

- a decision's `visa` is prose ("Articles L. 461-1 et L. 461-3 du code de l'urbanisme") whose link is a Légifrance **search** URL, never a `LEGIARTI` id, so the join is on normalised references, not ids;
- `Document.visa_refs` holds those keys and is filterable, so an article can list the decisions applying it, and a decision can resolve its articles.

Subdivision numbers ("alinéa 1", "§ 3") must not be read as article numbers — `refs.rs` strips them.

## Gotchas learned the hard way

- Meilisearch document ids allow only letters, digits, `-` and `_`. Chunk ids use `<decision id>_<n>`.
- `create_index` returns a *failed task* when the index exists; treat `index_already_exists` as success.
- Judilibre `/export` requires `batch` alongside `batch_size`, and rejects a PISTE key with `403` until the application is subscribed to the API for that environment.
- Meilisearch chat streams `_meiliSearchProgress` with `function_arguments` and `_meiliSearchSources` with `sources` (not `documents`). Sources are whole index documents; trim them client-side.
- The GPT-5.6 family cannot be used for chat: it refuses function tools unless `reasoning_effort` is `none`, which Meilisearch cannot send. Default model is `gpt-5.5`.
- Base UI `CollapsibleTrigger render={<Button/>}` causes a hydration mismatch; use `className={cn(buttonVariants(...))}` on the trigger instead.

## Conventions

- Search highlighting uses plain-text markers (`__hl__`) rendered by React, never `innerHTML`.
- The browser searches Meilisearch directly with a search-only key from `/api/config`, which must cover both indexes; chat goes through `/api/chat` (SSE passthrough).
- Judilibre text fields may contain HTML (notably `visa`); strip it in the indexer.
- UI copy is French; code, comments and docs are English.
