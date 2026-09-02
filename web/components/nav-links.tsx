"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { MessageSquareText, Search } from "lucide-react";
import { cn } from "@/lib/utils";

const links = [
  { href: "/", label: "Rechercher", icon: Search },
  { href: "/chat", label: "Assistant", icon: MessageSquareText },
];

export function NavLinks() {
  const pathname = usePathname();
  return (
    <nav aria-label="Navigation principale" className="flex items-center gap-1">
      {links.map(({ href, label, icon: Icon }) => {
        const active = href === "/" ? pathname === "/" || pathname.startsWith("/decision") : pathname.startsWith(href);
        return (
          <Link
            key={href}
            href={href}
            aria-current={active ? "page" : undefined}
            className={cn(
              "flex h-8 items-center gap-1.5 rounded-lg px-3 text-sm font-medium transition-colors",
              "focus-visible:ring-ring/50 outline-none focus-visible:ring-3",
              active ? "bg-accent text-accent-foreground" : "text-muted-foreground hover:text-foreground hover:bg-accent/60",
            )}
          >
            <Icon className="size-4" aria-hidden />
            {label}
          </Link>
        );
      })}
    </nav>
  );
}
