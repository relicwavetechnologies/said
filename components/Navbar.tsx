'use client';

import Link from 'next/link';
import { motion, useMotionValueEvent, useScroll } from 'motion/react';
import { useState } from 'react';
import { MagneticButton } from './motion/MagneticButton';

const NAV_LINKS = [
  { label: 'Pricing', href: '/pricing' },
  { label: 'User Guide', href: '/guide' },
  { label: 'Blog', href: '/blog' },
  { label: 'Changelog', href: '/changelog' },
  { label: 'API', href: '/api' },
  { label: 'Download iOS', href: '/ios' },
] as const;

export function Navbar() {
  const { scrollY } = useScroll();
  const [scrolled, setScrolled] = useState(false);
  useMotionValueEvent(scrollY, 'change', (v) => setScrolled(v > 40));

  return (
    <motion.header
      initial={{ y: -28, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ duration: 0.7, ease: [0.22, 1, 0.36, 1] }}
      className={`sticky top-0 z-40 w-full transition-all duration-300 ${
        scrolled
          ? 'bg-background/70 backdrop-blur-md shadow-[0_4px_24px_-12px_rgba(15,20,40,0.18)]'
          : 'bg-transparent'
      }`}
    >
      <div
        className={`mx-auto flex w-full max-w-[1280px] items-center justify-between px-token-md transition-all duration-300 ${
          scrolled ? 'py-3' : 'py-6'
        }`}
      >
        <Link
          href="/"
          aria-label="Aqua — home"
          className="text-[22px] font-bold tracking-[-0.04em] text-ink"
        >
          AQUA
        </Link>

        <nav
          aria-label="Primary"
          className="hidden items-center gap-1 rounded-full bg-[#eef0f3]/80 p-1.5 ring-1 ring-black/[0.04] backdrop-blur md:flex"
        >
          {NAV_LINKS.map((link) => (
            <Link
              key={link.href}
              href={link.href}
              className="group relative rounded-full px-4 py-2 text-[15px] font-medium text-muted transition-colors duration-300 hover:text-text"
            >
              <span className="relative">
                {link.label}
                <span className="absolute -bottom-[3px] left-0 h-[1.5px] w-full origin-left scale-x-0 rounded-full bg-text transition-transform duration-300 ease-out group-hover:scale-x-100" />
              </span>
            </Link>
          ))}
          <MagneticButton
            href="/download"
            className="ml-1 inline-flex items-center justify-center rounded-[10px] bg-accent px-5 py-2 text-[15px] font-semibold text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.55),0_1px_2px_rgba(13,71,161,0.18),0_6px_16px_rgba(103,190,255,0.32)] transition hover:brightness-105 active:brightness-95"
          >
            Download
          </MagneticButton>
        </nav>

        <Link
          href="/download"
          className="rounded-[10px] bg-accent px-4 py-2 text-[14px] font-semibold text-white shadow-glow md:hidden"
        >
          Download
        </Link>
      </div>
    </motion.header>
  );
}
