'use client';

import { motion } from 'motion/react';

type Props = {
  children: React.ReactNode;
  speed?: number;
  className?: string;
  pauseOnHover?: boolean;
};

export function Marquee({ children, speed = 28, className = '', pauseOnHover = true }: Props) {
  return (
    <div className={`group relative overflow-hidden ${className}`}>
      <div
        className="pointer-events-none absolute inset-y-0 left-0 z-10 w-16 bg-gradient-to-r from-background to-transparent"
        aria-hidden
      />
      <div
        className="pointer-events-none absolute inset-y-0 right-0 z-10 w-16 bg-gradient-to-l from-background to-transparent"
        aria-hidden
      />
      <motion.div
        className="flex w-max gap-12"
        animate={{ x: ['0%', '-50%'] }}
        transition={{ duration: speed, ease: 'linear', repeat: Infinity }}
        style={pauseOnHover ? { animationPlayState: 'running' } : undefined}
      >
        <div className="flex shrink-0 items-center gap-12">{children}</div>
        <div className="flex shrink-0 items-center gap-12" aria-hidden>
          {children}
        </div>
      </motion.div>
    </div>
  );
}
