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
    <header className="relative z-10 mx-auto flex w-full max-w-[1280px] items-center justify-between px-token-md pt-token-md">
      <Link
        href="/"
        aria-label="Aqua — home"
        className="text-[22px] font-medium tracking-tight text-text"
      >
        AQUA
      </Link>

      <nav
        aria-label="Primary"
        className="flex items-center gap-1 rounded-full bg-white/60 px-2 py-1.5 shadow-card backdrop-blur-md ring-1 ring-black/5"
      >
        {NAV_LINKS.map((link) => (
          <Link
            key={link.href}
            href={link.href}
            className="rounded-full px-3 py-1.5 text-[14px] text-muted transition hover:text-text"
          >
            {link.label}
          </Link>
        ))}
        <Link
          href="/download"
          className="ml-1 rounded-md bg-accent px-4 py-2 text-[14px] font-medium text-accent-text shadow-button transition hover:brightness-105 active:brightness-95"
        >
          Download
        </Link>
      </nav>
    </header>
  );
}
