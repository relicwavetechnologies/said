'use client';

import { motion, useScroll, useSpring } from 'motion/react';

export function ScrollProgress() {
  const { scrollYProgress } = useScroll();
  const scaleY = useSpring(scrollYProgress, { stiffness: 90, damping: 22, mass: 0.3 });

  return (
    <motion.div
      aria-hidden
      className="fixed right-2 top-0 z-50 h-screen w-[2px] origin-top bg-accent/80 shadow-[0_0_8px_rgba(103,190,255,0.6)]"
      style={{ scaleY }}
    />
  );
}
