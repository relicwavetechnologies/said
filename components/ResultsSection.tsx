'use client';

import { motion } from 'motion/react';
import { CountUp } from './motion/CountUp';
import { GradientMesh } from './motion/GradientMesh';
import { fadeUp, staggerParent, EASE_OUT, VIEWPORT_ONCE } from '@/lib/motion';

export function ResultsSection() {
  return (
    <section className="relative isolate w-full overflow-hidden bg-[#0a0b0f] text-white">
      <GradientMesh variant="dark" />
      <div
        className="aurora-sweep pointer-events-none absolute -top-32 left-0 right-0 h-[420px] bg-[radial-gradient(ellipse_at_center,rgba(103,190,255,0.35),transparent_60%)] blur-3xl"
        aria-hidden
      />

      <div className="relative mx-auto w-full max-w-[1280px] px-token-md pb-token-xl pt-24">
        <motion.div
          variants={staggerParent(0.12)}
          initial="hidden"
          whileInView="show"
          viewport={VIEWPORT_ONCE}
          className="max-w-[600px]"
        >
          <motion.p
            variants={fadeUp}
            className="text-[12.5px] uppercase tracking-[0.2em] text-white/45"
          >
            Real-world impact
          </motion.p>
          <motion.h2
            variants={fadeUp}
            className="mt-3 text-[36px] font-normal leading-[1.05] tracking-[-0.02em] sm:text-[44px] md:text-[52px] lg:text-[56px]"
          >
            Results you{' '}
            <span className="font-display text-accent">notice</span>
            <br />
            immediately
          </motion.h2>
          <motion.p
            variants={fadeUp}
            className="mt-5 max-w-[460px] text-[16px] leading-[1.55] text-white/60"
          >
            Aqua helps developers ship faster, stay focused, and spend less time
            on repetitive typing.
          </motion.p>
        </motion.div>

        <motion.div
          variants={staggerParent(0.18)}
          initial="hidden"
          whileInView="show"
          viewport={VIEWPORT_ONCE}
          className="mt-16 grid grid-cols-1 gap-x-16 gap-y-10 md:mt-20 md:grid-cols-2"
        >
          <motion.div variants={fadeUp}>
            <Stat
              valueNode={
                <>
                  <CountUp to={6} duration={1.4} />
                  <span className="mx-2 text-white/40">h</span>
                  <CountUp to={23} duration={1.6} />
                  <span className="ml-1 text-white/40">m</span>
                </>
              }
              label="Saved coding weekly"
            />
          </motion.div>
          <motion.div variants={fadeUp}>
            <Stat
              valueNode={
                <>
                  <CountUp to={230} duration={1.6} />
                  <span className="ml-1 text-white/40">wpm</span>
                </>
              }
              label="Write 5 times faster"
            />
          </motion.div>
        </motion.div>
      </div>
    </section>
  );
}

function Stat({ valueNode, label }: { valueNode: React.ReactNode; label: string }) {
  return (
    <div className="flex flex-col">
      <div className="flex items-end justify-between gap-6 pb-4">
        <span className="text-[44px] font-normal leading-[1] tracking-[-0.03em] tabular-nums text-white sm:text-[56px] md:text-[64px] lg:text-[80px]">
          {valueNode}
        </span>
        <span className="pb-2 text-[13px] text-white/55">{label}</span>
      </div>
      <motion.div
        className="h-px w-full origin-left bg-gradient-to-r from-accent via-white/30 to-transparent"
        initial={{ scaleX: 0 }}
        whileInView={{ scaleX: 1 }}
        viewport={{ once: true, amount: 0.4 }}
        transition={{ duration: 1.0, ease: EASE_OUT, delay: 0.3 }}
      />
    </div>
  );
}
