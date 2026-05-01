import Link from 'next/link';

const NAV_LINKS = [
  { label: 'Pricing', href: '/pricing' },
  { label: 'User Guide', href: '/guide' },
  { label: 'Blog', href: '/blog' },
  { label: 'Changelog', href: '/changelog' },
  { label: 'API', href: '/api' },
  { label: 'Download iOS', href: '/ios' },
] as const;

export function Navbar() {
  return (
    <header className="relative z-10 mx-auto flex w-full max-w-[1280px] items-center justify-between px-token-md pt-8">
      <Link
        href="/"
        aria-label="Aqua — home"
        className="text-[22px] font-semibold tracking-[-0.02em] text-text"
      >
        AQUA
      </Link>

      <nav
        aria-label="Primary"
        className="flex items-center gap-1 rounded-full bg-white/70 p-1.5 shadow-[0_2px_12px_rgba(41,44,61,0.06)] ring-1 ring-black/[0.04] backdrop-blur-md"
      >
        {NAV_LINKS.map((link) => (
          <Link
            key={link.href}
            href={link.href}
            className="rounded-full px-3.5 py-2 text-[14px] font-medium text-text/75 transition hover:text-text"
          >
            {link.label}
          </Link>
        ))}
        <Link
          href="/download"
          className="ml-1 rounded-[10px] bg-accent px-5 py-2 text-[14px] font-semibold text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.45),0_1px_2px_rgba(13,71,161,0.20),0_4px_12px_rgba(103,190,255,0.30)] transition hover:brightness-105 active:brightness-95"
        >
          Download
        </Link>
      </nav>
    </header>
  );
}
