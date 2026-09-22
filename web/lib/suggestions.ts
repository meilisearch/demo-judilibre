import type { SearchScope } from "@/lib/search-store";

/**
 * Queries shown in an empty search bar on first load, one per tab. Each was checked
 * against the live indexes: the top hits are the landmark article or the leading
 * line of case law, so the first screen already shows what the search can do.
 */
export const SUGGESTIONS: Record<SearchScope, readonly string[]> = {
  articles: [
    "harcèlement moral au travail",
    "responsabilité du fait des choses",
    "garantie des vices cachés de la chose vendue",
    "trouble anormal de voisinage",
    "le propriétaire peut-il couper les branches du voisin",
    "durée maximale de la période d'essai",
    "rupture conventionnelle du contrat de travail",
    "autorité parentale",
    "réserve héréditaire",
    "abus de confiance",
  ],
  decisions: [
    "prise d'acte de la rupture",
    "faute inexcusable de l'employeur",
    "harcèlement moral",
    "troubles anormaux du voisinage",
    "devoir de mise en garde du banquier",
    "accident de la circulation indemnisation",
    "rupture brutale des relations commerciales",
    "convention de forfait en jours",
    "responsabilité du notaire",
    "refus de renouvellement du bail commercial",
  ],
};

export function randomSuggestion(scope: SearchScope): string {
  const list = SUGGESTIONS[scope];
  return list[Math.floor(Math.random() * list.length)];
}
