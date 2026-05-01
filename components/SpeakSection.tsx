'use client';

import { motion, useScroll, useTransform, useInView } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { TrustLogos } from './TrustLogos';
import { Reveal } from './motion/Reveal';
import { MagneticButton } from './motion/MagneticButton';
import { fadeUp, staggerParent, EASE_OUT, VIEWPORT_ONCE } from '@/lib/motion';

const DICTATION_TEXT = `The rain had finally eased, leaving the streets washed in silver light. Elena pulled her coat tighter, not against the cold, but against the strange feeling that someone had been following her since she left the café.

She glanced over her shoulder—nothing. Just the quiet rhythm of the city, the drip of water from iron balconies, the hum of distant traffic. Still, the unease clung to her.`;

export function SpeakSection() {
  const ref = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ['start end', 'end start'] });
  const monitorScale = useTransform(scrollYProgress, [0, 0.4, 1], [0.92, 1, 1.02]);
  const monitorY = useTransform(scrollYProgress, [0, 1], [40, -40]);

  return (
    <section className="relative w-full">
      <div className="mx-auto w-full max-w-[1280px] px-token-md py-token-xl text-center">
        <motion.div
          variants={staggerParent(0.12)}
          initial="hidden"
          whileInView="show"
          viewport={VIEWPORT_ONCE}
        >
          <motion.h2
            variants={fadeUp}
            className="mx-auto text-[44px] font-normal leading-[1.06] tracking-[-0.02em] text-text sm:text-[52px] md:text-[60px] lg:text-[64px]"
          >
            Speak, <span className="font-display text-accent-deep">and it’s done</span>
          </motion.h2>

          <motion.p
            variants={fadeUp}
            className="mx-auto mt-6 max-w-[560px] text-[18px] leading-[1.55] text-text/80 md:text-[20px]"
          >
            Speak naturally, and let Aqua’s AI refine your words as you talk.
            <br />
            Fast, accurate, and works with every app.
          </motion.p>

          <motion.div variants={fadeUp} className="mt-10 flex justify-center">
            <MagneticButton
              href="/download"
              className="inline-flex items-center justify-center rounded-full bg-[#eef0f3] px-7 py-3 text-[15px] font-medium text-text transition hover:bg-[#e4e7ec] active:bg-[#dde0e6]"
            >
              Start transcribing
            </MagneticButton>
          </motion.div>
        </motion.div>

        <motion.div
          ref={ref}
          className="relative mt-16 md:mt-20"
          style={{ scale: monitorScale, y: monitorY }}
        >
          <MonitorMockup />
        </motion.div>

        <Reveal className="mt-12 flex flex-wrap items-center justify-between gap-y-6 text-left">
          <div className="flex items-center gap-3 text-[15px] text-text/85">
            <span>Hold</span>
            <kbd className="inline-flex h-9 min-w-[72px] items-center justify-center rounded-[8px] bg-white px-3.5 font-sans text-[13px] font-medium text-text shadow-[0_1px_0_rgba(0,0,0,0.04),0_4px_10px_rgba(41,44,61,0.08)] ring-1 ring-black/[0.06]">
              Space
            </kbd>
            <span>and try yourself</span>
          </div>

          <div className="flex-1 min-w-[280px]">
            <TrustLogos />
          </div>
        </Reveal>
      </div>
    </section>
  );
}

function MonitorMockup() {
  const stageRef = useRef<HTMLDivElement>(null);
  const inView = useInView(stageRef, { amount: 0.5, once: false });
  const { typed, done } = useDictationStream(DICTATION_TEXT, inView);

  const charCount = typed.length;
  const wordCount = typed.trim() ? typed.trim().split(/\s+/).length : 0;

  return (
    <div ref={stageRef} className="mx-auto w-full max-w-[980px]">
      <div className="shimmer-overlay relative overflow-hidden rounded-[24px] border border-black/10 bg-white p-3 shadow-[0_30px_80px_rgba(41,44,61,0.18)]">
        <div className="relative aspect-[16/10] overflow-hidden rounded-[14px]">
          <div
            aria-hidden
            className="absolute inset-0"
            style={{
              backgroundImage:
                'linear-gradient(120deg, #cfe0f3 0%, #d6e4f5 30%, #e7eef8 55%, #cfdef0 80%, #b9cde3 100%)',
            }}
          />
          <motion.div
            aria-hidden
            className="absolute inset-0 opacity-70"
            style={{
              backgroundImage:
                "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 800 500'><defs><filter id='w'><feTurbulence type='fractalNoise' baseFrequency='0.008' numOctaves='2' seed='4'/><feDisplacementMap in='SourceGraphic' scale='40'/></filter></defs><g filter='url(%23w)'><rect width='100%25' height='100%25' fill='url(%23g)'/></g><defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='%23bcd1ea'/><stop offset='1' stop-color='%23e6eef8'/></linearGradient></defs></svg>\")",
              backgroundSize: 'cover',
            }}
            animate={{ backgroundPositionX: ['0%', '100%', '0%'] }}
            transition={{ duration: 24, repeat: Infinity, ease: 'easeInOut' }}
          />

          <div className="absolute inset-x-[6%] top-[8%] bottom-[8%] overflow-hidden rounded-[14px] bg-white shadow-[0_18px_40px_rgba(41,44,61,0.10)]">
            <AppToolbar charCount={charCount} wordCount={wordCount} />
            <Transcript typed={typed} done={done} />
          </div>

          <DictationPill active={!done} />

          <div className="absolute inset-x-0 bottom-3 flex justify-center">
            <span className="h-[5px] w-[100px] rounded-full bg-black/40" aria-hidden />
          </div>
        </div>
      </div>

      <div className="mx-auto h-5 w-[160px] rounded-b-[12px] bg-gradient-to-b from-[#d4d8de] to-[#bfc4cc]" />
      <div className="mx-auto h-3 w-[260px] rounded-b-[20px] bg-gradient-to-b from-[#bfc4cc] to-[#a9aeb6] shadow-[0_8px_18px_rgba(41,44,61,0.08)]" />
    </div>
  );
}

function useDictationStream(text: string, active: boolean) {
  const [typed, setTyped] = useState('');
  const [done, setDone] = useState(false);

  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    let i = 0;
    setTyped('');
    setDone(false);

    const tick = () => {
      if (cancelled) return;
      if (i >= text.length) {
        setDone(true);
        // Loop: pause, then restart so the demo replays while visible
        setTimeout(() => {
          if (cancelled) return;
          i = 0;
          setTyped('');
          setDone(false);
          setTimeout(tick, 600);
        }, 3200);
        return;
      }
      i++;
      setTyped(text.slice(0, i));
      const ch = text[i - 1];
      let delay = 18 + Math.random() * 22;
      if (ch === ' ') delay = 28 + Math.random() * 20;
      else if (ch === ',') delay = 160 + Math.random() * 60;
      else if (ch === '.') delay = 260 + Math.random() * 80;
      else if (ch === '\n') delay = 220;
      else if (ch === '—') delay = 180;
      setTimeout(tick, delay);
    };

    const startId = setTimeout(tick, 700);
    return () => {
      cancelled = true;
      clearTimeout(startId);
    };
  }, [text, active]);

  return { typed, done };
}

function AppToolbar({ charCount, wordCount }: { charCount: number; wordCount: number }) {
  return (
    <div className="flex items-center justify-between px-6 pt-5">
      <button
        aria-label="Menu"
        className="flex h-9 w-9 items-center justify-center rounded-full ring-1 ring-black/10"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
          <path d="M4 7h16M4 12h16M4 17h16" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      </button>

      <div className="rounded-full ring-1 ring-black/10 px-5 py-1.5 font-mono text-[13px] tabular-nums text-text/80">
        {charCount.toLocaleString()} characters &nbsp;&nbsp;&nbsp; {wordCount.toLocaleString()} words
      </div>

      <button
        aria-label="Share"
        className="flex h-9 w-9 items-center justify-center rounded-full ring-1 ring-black/10"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
          <path
            d="M12 4v12M12 4l-4 4M12 4l4 4M5 14v4a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-4"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>
    </div>
  );
}

function Transcript({ typed, done }: { typed: string; done: boolean }) {
  const paragraphs = typed.split('\n\n');
  return (
    <motion.div
      className="px-10 pb-20 pt-8 text-left font-mono text-[13.5px] leading-[1.85] text-text"
      initial={{ opacity: 0 }}
      whileInView={{ opacity: 1 }}
      viewport={{ once: true, amount: 0.4 }}
      transition={{ duration: 0.5, ease: EASE_OUT }}
    >
      <h3 className="mb-4 text-[15px] font-semibold text-text">Chapter 2</h3>
      {paragraphs.map((p, i) => (
        <p key={i} className={i > 0 ? 'mt-5' : ''}>
          {p}
          {!done && i === paragraphs.length - 1 && <Caret />}
        </p>
      ))}
      {paragraphs.length === 0 && !done && (
        <p>
          <Caret />
        </p>
      )}
    </motion.div>
  );
}

function Caret() {
  return (
    <span
      aria-hidden
      className="ml-[1px] inline-block h-[1.05em] w-[2px] -translate-y-[1px] animate-pulse bg-accent-deep align-middle"
    />
  );
}

function DictationPill({ active }: { active: boolean }) {
  return (
    <motion.div
      aria-hidden
      className="absolute inset-x-0 bottom-10 flex justify-center"
      initial={{ y: 12, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ duration: 0.5, delay: 0.2, ease: EASE_OUT }}
    >
      <motion.div
        className="flex items-center gap-3 rounded-full bg-[#0d0e12] px-3.5 py-2 shadow-[0_18px_40px_rgba(0,0,0,0.35),inset_0_1px_0_rgba(255,255,255,0.08)] ring-1 ring-white/5"
        animate={active ? { scale: [1, 1.02, 1] } : { scale: 1 }}
        transition={{ duration: 2.2, repeat: Infinity, ease: 'easeInOut' }}
      >
        <span className="relative flex h-3.5 w-3.5 items-center justify-center">
          {active && (
            <span className="absolute h-full w-full rounded-full bg-accent/60 animate-ping" />
          )}
          <span
            className={`h-3 w-3 rounded-full ${active ? 'bg-accent shadow-[0_0_10px_rgba(103,190,255,0.8)]' : 'bg-white/30'}`}
          />
        </span>
        <div className="flex h-5 items-center gap-[2.5px]">
          {Array.from({ length: 22 }).map((_, i) => (
            <motion.span
              key={i}
              className="w-[2.5px] rounded-full bg-white/85"
              animate={
                active
                  ? { height: [4, 6 + (i % 5) * 3, 4, 10 + (i % 3) * 2, 5] }
                  : { height: 4 }
              }
              transition={{
                duration: 1.0 + (i % 5) * 0.18,
                repeat: Infinity,
                ease: 'easeInOut',
                delay: i * 0.04,
              }}
              style={{ height: 5 }}
            />
          ))}
        </div>
      </motion.div>
    </motion.div>
  );
}
