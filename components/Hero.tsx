'use client';

import Image from 'next/image';
import { motion, useScroll, useTransform } from 'motion/react';
import { useRef } from 'react';
import { GradientMesh } from './motion/GradientMesh';
import { MagneticButton } from './motion/MagneticButton';
import { staggerParent, wordReveal, fadeUp, EASE_OUT } from '@/lib/motion';

const LINE_1 = ['We’ve', 'typed', 'for', '150', 'years.'];
const LINE_2 = ['It’s', 'time', 'to'];

export function Hero() {
  const ref = useRef<HTMLElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ['start start', 'end start'] });
  const photoY = useTransform(scrollYProgress, [0, 1], [0, 80]);
  const photoOpacity = useTransform(scrollYProgress, [0, 0.6], [1, 0.4]);

  return (
    <section ref={ref} className="relative isolate w-full overflow-hidden">
      <div className="water-bg absolute inset-0 -z-20" aria-hidden />
      <GradientMesh className="-z-10" />

      <div className="mx-auto grid w-full max-w-[1280px] grid-cols-1 gap-x-12 px-token-md pb-token-xl pt-[100px] md:pt-[140px] lg:grid-cols-12">
        <div className="lg:col-span-8">
          <motion.h1
            className="text-[40px] font-normal leading-[1.06] tracking-[-0.02em] text-text sm:text-[48px] md:text-[56px] lg:text-[64px]"
            variants={staggerParent(0.06, 0.05)}
            initial="hidden"
            animate="show"
          >
            <span className="block">
              {LINE_1.map((w, i) => (
                <span key={i} className="inline-block overflow-hidden align-bottom">
                  <motion.span variants={wordReveal} className="mr-[0.25em] inline-block">
                    {w}
                  </motion.span>
                </span>
              ))}
            </span>
            <span className="block">
              {LINE_2.map((w, i) => (
                <span key={i} className="inline-block overflow-hidden align-bottom">
                  <motion.span variants={wordReveal} className="mr-[0.25em] inline-block">
                    {w}
                  </motion.span>
                </span>
              ))}
              <span className="inline-block overflow-hidden align-bottom">
                <motion.span
                  variants={wordReveal}
                  className="font-display inline-block text-accent-deep"
                >
                  speak.
                </motion.span>
              </span>
            </span>
          </motion.h1>

          <motion.p
            className="mt-8 max-w-[560px] text-[18px] leading-[1.55] text-text/85 md:text-[22px] md:leading-[1.5]"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.55, ease: EASE_OUT }}
          >
            Aqua turns your voice into clear text in real time,
            <br />
            for everything from AI prompts to essays.
            <span
              aria-hidden
              className="pulse-dot ml-2 inline-block h-3 w-3 translate-y-[1px] rounded-full bg-accent align-middle shadow-glow"
            />
          </motion.p>

          <motion.div
            className="mt-12 flex flex-wrap items-center gap-x-4 gap-y-4 text-[15px] text-text/85"
            initial={{ opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.75, ease: EASE_OUT }}
          >
            <MagneticButton
              href="/download"
              className="inline-flex items-center justify-center rounded-full bg-ink px-7 py-3.5 text-[15px] font-medium text-white shadow-[0_10px_30px_-12px_rgba(0,0,0,0.55)] transition hover:bg-text"
            >
              Download for iOS
            </MagneticButton>

            <span className="ml-2 flex items-center gap-2.5 text-text/70">
              <span>or hold</span>
              <kbd className="inline-flex h-9 min-w-[72px] items-center justify-center rounded-[8px] bg-white px-3.5 font-sans text-[13px] font-medium text-text shadow-[0_1px_0_rgba(0,0,0,0.04),0_4px_10px_rgba(41,44,61,0.08)] ring-1 ring-black/[0.06]">
                Space
              </kbd>
              <span>to dictate</span>
            </span>
          </motion.div>
        </div>

        <motion.div
          className="relative mt-12 hidden lg:col-span-4 lg:mt-0 lg:block"
          style={{ y: photoY, opacity: photoOpacity }}
          variants={fadeUp}
          initial="hidden"
          animate="show"
          transition={{ duration: 1.0, delay: 0.4, ease: EASE_OUT }}
        >
          <div className="relative h-full min-h-[420px] overflow-hidden rounded-[28px] shadow-card-hover ring-1 ring-black/5">
            <Image
              src="/images/hero-workspace.jpg"
              alt="Quiet workspace at dawn"
              fill
              priority
              sizes="(min-width: 1024px) 33vw, 100vw"
              className="object-cover"
            />
            <div
              className="absolute inset-0 bg-gradient-to-t from-background/70 via-transparent to-transparent"
              aria-hidden
            />
            <div className="absolute bottom-4 left-4 right-4 rounded-2xl bg-white/80 p-4 backdrop-blur ring-1 ring-black/5">
              <div className="flex items-center gap-3">
                <span className="pulse-dot h-2.5 w-2.5 rounded-full bg-accent shadow-glow" />
                <span className="text-[13px] font-medium text-text">Listening…</span>
                <span className="ml-auto font-mono text-[12px] tabular-nums text-muted">
                  230 wpm
                </span>
              </div>
              <div className="mt-3 flex items-end gap-1">
                {Array.from({ length: 24 }).map((_, i) => (
                  <motion.span
                    key={i}
                    className="w-[3px] rounded-full bg-accent"
                    animate={{ height: [4, 14 + (i % 5) * 4, 6, 12, 4] }}
                    transition={{
                      duration: 1.6 + (i % 4) * 0.2,
                      repeat: Infinity,
                      ease: 'easeInOut',
                      delay: i * 0.04,
                    }}
                    style={{ height: 6 }}
                  />
                ))}
              </div>
            </div>
          </div>
        </motion.div>
      </div>
    </section>
  );
}
