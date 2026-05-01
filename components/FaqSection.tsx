'use client';

import { useState } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { Reveal } from './motion/Reveal';
import { fadeUp, staggerParent, EASE_OUT, VIEWPORT_ONCE } from '@/lib/motion';

const FAQS = [
  {
    q: 'How accurate is Aqua compared to typing?',
    a: 'Aqua benchmarks at over 98% word accuracy across natural speech, technical terms, and code identifiers — typically twice as accurate as a human typist working at speed.',
  },
  {
    q: 'Does it work with the apps I already use?',
    a: 'Yes. Aqua sits at the system layer, so anywhere you can type — Slack, Gmail, Notion, VS Code, Claude, Cursor, your terminal — Aqua works.',
  },
  {
    q: 'What happens to my voice data?',
    a: 'Audio is processed in real time and never stored on our servers. Transcripts stay on your device unless you explicitly export them.',
  },
  {
    q: 'Which platforms are supported today?',
    a: 'Aqua is live on iOS and macOS. Windows and Android are in active development — join the waitlist to get early access.',
  },
  {
    q: 'Do you offer a free trial?',
    a: 'Every new account gets unlimited dictation for the first 7 days. After that you can keep a free 30-minute daily allowance or upgrade to Pro for unlimited.',
  },
];

export function FaqSection() {
  return (
    <section className="relative w-full">
      <div className="mx-auto w-full max-w-[920px] px-token-md py-token-xl">
        <motion.div
          variants={staggerParent(0.1)}
          initial="hidden"
          whileInView="show"
          viewport={VIEWPORT_ONCE}
          className="text-center"
        >
          <motion.p
            variants={fadeUp}
            className="text-[12.5px] uppercase tracking-[0.2em] text-muted"
          >
            FAQ
          </motion.p>
          <motion.h2
            variants={fadeUp}
            className="mt-3 text-[36px] font-normal leading-[1.06] tracking-[-0.02em] text-text sm:text-[44px] md:text-[52px]"
          >
            Questions, <span className="font-display text-accent-deep">answered</span>
          </motion.h2>
        </motion.div>

        <Reveal className="mt-14 divide-y divide-black/[0.08] rounded-[20px] bg-white/60 px-2 ring-1 ring-black/[0.05] backdrop-blur">
          {FAQS.map((item, i) => (
            <FaqItem key={item.q} item={item} index={i} />
          ))}
        </Reveal>
      </div>
    </section>
  );
}

function FaqItem({ item, index }: { item: { q: string; a: string }; index: number }) {
  const [open, setOpen] = useState(index === 0);
  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="flex w-full items-center justify-between gap-6 px-5 py-5 text-left transition-colors hover:bg-black/[0.02] sm:px-6"
      >
        <span className="text-[17px] font-medium text-text sm:text-[18px]">{item.q}</span>
        <motion.span
          aria-hidden
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-accent/10 text-accent-deep"
          animate={{ rotate: open ? 45 : 0 }}
          transition={{ duration: 0.35, ease: EASE_OUT }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M12 5v14M5 12h14" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
        </motion.span>
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            key="content"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.4, ease: EASE_OUT }}
            className="overflow-hidden"
          >
            <p className="px-5 pb-6 text-[15px] leading-[1.65] text-muted sm:px-6">{item.a}</p>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
