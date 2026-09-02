import { format, parseISO } from "date-fns";
import { fr } from "date-fns/locale";

export function formatDate(iso: string, pattern = "d MMMM yyyy"): string {
  if (!iso) return "";
  try {
    return format(parseISO(iso), pattern, { locale: fr });
  } catch {
    return iso;
  }
}

/** Conventional French citation, e.g. "Cass. soc., 12 janvier 2024, n° 22-10.123". */
export function citation(d: { jurisdiction: string; chamber: string; decision_date: string; number: string }): string {
  const court = shortJurisdiction(d.jurisdiction);
  const chamber = shortChamber(d.chamber);
  const parts = [chamber ? `${court} ${chamber}` : court, formatDate(d.decision_date)];
  if (d.number) parts.push(`n° ${d.number}`);
  return parts.join(", ");
}

export function shortJurisdiction(j: string): string {
  const s = j.toLowerCase();
  if (s.includes("cassation") || s === "cc") return "Cass.";
  if (s.includes("appel") || s === "ca") return "CA";
  if (s.includes("judiciaire") || s === "tj") return "TJ";
  if (s.includes("commerce") || s === "tcom") return "T. com.";
  return j;
}

const CHAMBERS: Array<[RegExp, string]> = [
  [/premi[eè]re chambre civile|civ1/i, "1re civ."],
  [/deuxi[eè]me chambre civile|civ2/i, "2e civ."],
  [/troisi[eè]me chambre civile|civ3/i, "3e civ."],
  [/chambre commerciale|comm/i, "com."],
  [/chambre sociale|soc/i, "soc."],
  [/chambre criminelle|crim/i, "crim."],
  [/chambre mixte/i, "ch. mixte"],
  [/assembl[ée]e pl[ée]ni[èe]re/i, "ass. plén."],
  [/ordonnance|ordo/i, "ord."],
  [/avis/i, "avis"],
];

export function shortChamber(chamber: string): string {
  for (const [re, short] of CHAMBERS) {
    if (re.test(chamber)) return short;
  }
  return chamber;
}

export function formatCount(n: number): string {
  return new Intl.NumberFormat("fr-FR").format(n);
}

export function facetLabel(attr: string): string {
  const labels: Record<string, string> = {
    chamber: "Chambre",
    solution: "Solution",
    publication: "Publication",
    type: "Nature",
    year: "Année",
    formation: "Formation",
    themes: "Matières",
    jurisdiction: "Juridiction",
  };
  return labels[attr] ?? attr;
}
