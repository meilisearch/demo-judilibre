import type { Metadata } from "next";
import { IBM_Plex_Mono, IBM_Plex_Sans, Newsreader } from "next/font/google";
import Link from "next/link";
import { Toaster } from "sonner";
import { Providers } from "@/components/providers";
import { NavLinks } from "@/components/nav-links";
import { SiteFooter } from "@/components/site-footer";
import "./globals.css";

const plexSans = IBM_Plex_Sans({
  variable: "--font-plex-sans",
  subsets: ["latin", "latin-ext"],
  weight: ["400", "500", "600"],
});

const plexMono = IBM_Plex_Mono({
  variable: "--font-plex-mono",
  subsets: ["latin"],
  weight: ["400", "500"],
});

const newsreader = Newsreader({
  variable: "--font-newsreader",
  subsets: ["latin", "latin-ext"],
  style: ["normal", "italic"],
});

export const metadata: Metadata = {
  title: { default: "Judilibre × Meilisearch", template: "%s · Judilibre × Meilisearch" },
  description:
    "Recherche instantanée et assistant conversationnel sur la jurisprudence française (Cour de cassation), propulsés par Meilisearch.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="fr" className={`${plexSans.variable} ${plexMono.variable} ${newsreader.variable} h-full antialiased`}>
      <body className="flex min-h-full flex-col">
        <Providers>
          <header className="h-(--header-height) border-b bg-background/90 backdrop-blur supports-[backdrop-filter]:bg-background/70 sticky top-0 z-20">
            <div className="mx-auto flex h-full w-full max-w-7xl items-center justify-between gap-6 px-4 sm:px-6">
              <Link href="/" className="flex items-baseline gap-2">
                <span className="font-heading text-2xl leading-none font-medium tracking-tight">Judilibre</span>
                <span className="text-muted-foreground text-xs font-medium tracking-wide uppercase">× Meilisearch</span>
              </Link>
              <NavLinks />
            </div>
          </header>
          <main className="flex flex-1 flex-col">{children}</main>
          <SiteFooter />
          <Toaster richColors position="bottom-right" />
        </Providers>
      </body>
    </html>
  );
}
