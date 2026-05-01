'use client';

import { motion, AnimatePresence } from 'motion/react';
import { useState } from 'react';

type Props = {
  message: string;
  ctaLabel: string;
  ctaHref: string;
};

export function AnnouncementBar({ message, ctaLabel, ctaHref }: Props) {
  const [open, setOpen] = useState(true);

  return (
    <AnimatePresence initial={false}>
      {open && (
        <motion.div
          key="bar"
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: 36, opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          transition={{ duration: 0.45, ease: [0.22, 1, 0.36, 1] }}
          className="w-full overflow-hidden bg-ink text-white"
        >
          <div className="relative mx-auto flex h-9 items-center justify-center gap-2 px-token-sm text-[14px]">
            <motion.span
              className="text-white/95"
              initial={{ y: 8, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              transition={{ delay: 0.2, duration: 0.5 }}
            >
              {message}
            </motion.span>
            <span aria-hidden className="text-white/50">
              —
            </span>
            <motion.a
              href={ctaHref}
              className="group relative font-medium text-white"
              initial={{ y: 8, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              transition={{ delay: 0.3, duration: 0.5 }}
            >
              <span>{ctaLabel}</span>
              <span className="absolute -bottom-[1px] left-0 h-px w-full origin-left scale-x-100 bg-white/80 transition-transform duration-300 group-hover:scale-x-110" />
            </motion.a>
            <button
              type="button"
              aria-label="Dismiss"
              onClick={() => setOpen(false)}
              className="absolute right-3 flex h-5 w-5 items-center justify-center rounded-full text-white/55 transition hover:bg-white/10 hover:text-white"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                <path d="M1 1l8 8M9 1L1 9" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
              </svg>
            </button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
