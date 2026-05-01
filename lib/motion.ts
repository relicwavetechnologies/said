import type { Variants, Transition } from 'motion/react';
import { easing, duration } from './tokens';

export const EASE_OUT: Transition['ease'] = [...easing.out];
export const EASE_IN: Transition['ease'] = [...easing.in];
export const EASE_SPRING: Transition['ease'] = [...easing.spring];

export const fadeUp: Variants = {
  hidden: { opacity: 0, y: 24 },
  show: {
    opacity: 1,
    y: 0,
    transition: { duration: duration.entrance, ease: EASE_OUT },
  },
};

export const fadeIn: Variants = {
  hidden: { opacity: 0 },
  show: {
    opacity: 1,
    transition: { duration: duration.entrance, ease: EASE_OUT },
  },
};

export const scaleIn: Variants = {
  hidden: { opacity: 0, scale: 0.94 },
  show: {
    opacity: 1,
    scale: 1,
    transition: { duration: duration.hero, ease: EASE_OUT },
  },
};

export const staggerParent = (stagger = 0.08, delay = 0): Variants => ({
  hidden: {},
  show: {
    transition: { staggerChildren: stagger, delayChildren: delay },
  },
});

export const wordReveal: Variants = {
  hidden: { opacity: 0, y: '0.6em' },
  show: {
    opacity: 1,
    y: 0,
    transition: { duration: 0.7, ease: EASE_OUT },
  },
};

export const VIEWPORT_ONCE = { once: true, amount: 0.25 } as const;
