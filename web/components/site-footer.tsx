"use client";

import { usePathname } from "next/navigation";

/**
 * Site footer. Hidden on the assistant, which fills the viewport below the header
 * and scrolls its conversation and its sources on their own.
 */
export function SiteFooter() {
  if (usePathname() === "/chat") return null;

  return (
    <footer className="border-t">
      <div className="text-muted-foreground mx-auto flex w-full max-w-7xl flex-col gap-1 px-4 py-4 text-xs sm:flex-row sm:items-center sm:justify-between sm:px-6">
        <p>
          Données : Cour de cassation, base Judilibre (Licence Ouverte 2.0), via l&apos;API PISTE. Textes pseudonymisés.
        </p>
        <p>Démonstration Meilisearch : recherche hybride et chat completions.</p>
      </div>
    </footer>
  );
}
