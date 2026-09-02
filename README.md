# Judilibre × Meilisearch

Demo: French case law (Cour de cassation decisions from the **Judilibre** open-data API) indexed in **Meilisearch**, with a Next.js UI for instant search and a conversational assistant powered by Meilisearch chat completions.

```
┌──────────────┐  /export   ┌──────────────┐  decisions  ┌─────────────────────┐
│ Judilibre    │ ─────────▶ │ indexer      │ ──────────▶ │ Meilisearch         │
│ API (PISTE)  │            │ (Rust CLI)   │  passages   │  judilibre          │
└──────────────┘            └──────┬───────┘ ──────────▶ │  judilibre_chunk    │
                                   │                     └──────┬──────────────┘
      attached PDFs ───────────────┘                     search ▲   │ chat completions (SSE)
      (pdf-inspector)                                           │   ▼
   embeddings: Voyage AI ────────────────────────▶      ┌──────────────┐
                                                        │ web (Next.js)│ / search · /chat · /decision/[id]
                                                        └──────────────┘
```

Two indexes: `judilibre` holds one document per decision, `judilibre_chunk` holds ~2 000-character passages of the decisions **and of their attached PDFs**, each embedded with Voyage AI. The search page switches between the two; the assistant retrieves from both.

## Prerequisites

| What | Why |
|------|-----|
| Docker (OrbStack) | runs Meilisearch (and optionally the web app) |
| Rust toolchain | builds the indexer (`cargo`) |
| Node 22 + pnpm | runs the web app locally |
| **PISTE credentials** | required to call the Judilibre API. Free: create an account on <https://piste.gouv.fr>, create an application, open its **APIs** tab and subscribe it to *Judilibre* (sandbox and/or production). Then copy its `KeyId` (API-key application) into `PISTE_KEY_ID`, or its OAuth client id/secret into `PISTE_CLIENT_ID` / `PISTE_CLIENT_SECRET`. A `403` means the application is not subscribed for that environment. |
| LLM API key | for the assistant. Default: OpenAI GPT-5.6 Luna (`CHAT_MODEL=gpt-5.6-luna`). Meilisearch also supports Mistral, Azure OpenAI, Gemini, vLLM. |
| Voyage AI key (optional) | hybrid semantic search with `voyage-law-2`. |

## Quick start

```bash
cp .env.example .env            # then fill PISTE_KEY_ID and CHAT_API_KEY
docker compose up -d meilisearch

cd indexer
cargo run --release -- setup                       # index settings + chat workspace
cargo run --release -- index --limit 3000          # most recent Cour de cassation decisions first
cd ..

cd web && pnpm install && pnpm dev                 # http://localhost:3000
```

Or run the web app in Docker with hot reload: `docker compose watch`.

## Indexer

`indexer/` is a Rust CLI. All flags are also read from environment variables (see `.env.example`).

```bash
cargo run -- setup [--no-chat]                     # create index, settings, embedder (optional), chat
cargo run -- index --date-start 2023-01-01 --date-end 2024-12-31 --jurisdiction cc,ca --limit 5000
cargo run -- index --updated --date-start 2025-01-01   # incremental sync: decisions updated since a date
```

- Export is windowed by month (most recent first) and windows are split when they exceed the Judilibre pagination cap (10 000 results).
- Decisions flagged `to_be_deleted` by Judilibre are removed from the index.
- Documents keep the full pseudonymised text plus an `excerpt` (motivations + dispositif) used by the assistant.
- `--publication b` restricts the export to Bulletin-published decisions: far richer titrage and sommaire, and a much smaller corpus (about 3 000 for 2024 to today, against 42 000 in total).
- `--with-files` downloads each attached PDF (communiqué, rapport du conseiller, avis de l'avocat général) and extracts its text with [`pdf-inspector`](https://github.com/firecrawl/pdf-inspector); the text is indexed with the decision and chunked into the passage index.
- Passages go to `<index>_chunk` unless `--no-chunks` is passed. Set `VOYAGE_API_KEY` before `setup` to embed both indexes with Voyage AI (`voyage-law-2` by default).

## Web app

`web/` is a Next.js App Router app (TypeScript, Tailwind v4, shadcn/ui, TanStack Query, Zustand).

- `/` search-as-you-type with facets (chamber, solution, publication, nature, year, formation, matières), a **Décisions / Passages** scope switch, an **IA** button toggling hybrid semantic search, sorting and highlighted matches. The browser talks to Meilisearch directly with a search-only key obtained from `/api/config`.
- `/chat` streaming assistant. `/api/chat` proxies to Meilisearch `POST /chats/{workspace}/chat/completions`; search steps and retrieved decisions are rendered as sources.
- `/decision/[id]` full decision: sommaire, titrage, textes appliqués, documents associés, rapprochements, texte intégral, and the text extracted from each attached PDF.

## Deploy

Only `web/` is deployed. The deployed app runs on two scoped Meilisearch keys and **no admin key**: a search-only key for the browser and a `documents.get` key for the decision page.

```bash
cd web && vercel deploy --prod
```

Live demo: <https://demo-judilibre.vercel.app>

## Docs

Mintlify documentation lives in `docs/` (`mintlify dev` inside that folder). The web API is described in `docs/openapi.yaml`.

## Data & licence

Judilibre data is published by the Cour de cassation under the Licence Ouverte 2.0 with Judilibre-specific terms of use; texts are pseudonymised. This project is a technical demo, not legal advice.
