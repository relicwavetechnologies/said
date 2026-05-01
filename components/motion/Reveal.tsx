'use client';

import { motion, type Variants } from 'motion/react';
import { fadeUp, VIEWPORT_ONCE } from '@/lib/motion';

type Props = {
  children: React.ReactNode;
  className?: string;
  delay?: number;
  variants?: Variants;
  as?: 'div' | 'section' | 'span' | 'li' | 'header' | 'p';
};

export function Reveal({
  children,
  className,
  delay = 0,
  variants = fadeUp,
  as = 'div',
}: Props) {
  const Comp = motion[as];
  return (
    <Comp
      className={className}
      variants={variants}
      initial="hidden"
      whileInView="show"
      viewport={VIEWPORT_ONCE}
      transition={{ delay }}
    >
      {children}
    </Comp>
  );
}
