/**
 * Centralized design tokens — the single source of truth for the Aqua landing page.
 * Tailwind config and runtime code both consume these values.
 */

export const colors = {
  accent: '#67beff',
  accentText: '#0000ee',
  background: '#fafbfc',
  text: '#292c3d',
  muted: '#7a7d88',
  ink: '#000000',
} as const;

export const fontFamily = {
  sans: [
    'PP Neue Montreal Book',
    'PP Neue Montreal',
    'var(--font-sans)',
    'Inter Tight',
    'Inter',
    '-apple-system',
    'BlinkMacSystemFont',
    'Segoe UI',
    'sans-serif',
  ],
} as const;

export const fontSize = {
  caption: ['0.75rem', { lineHeight: '1.1rem' }],
  label: ['0.875rem', { lineHeight: '1.25rem' }],
  body: ['1.5rem', { lineHeight: '2.4rem' }],
  h4: ['1.9375rem', { lineHeight: '2.4rem' }],
  h3: ['2.4375rem', { lineHeight: '2.875rem' }],
  h2: ['3.125rem', { lineHeight: '3.625rem' }],
  h1: ['4rem', { lineHeight: '4.5rem', letterSpacing: '-0.01em' }],
} as const;

export const spacing = {
  xs: '0.625rem',
  sm: '1.25rem',
  md: '2.5rem',
  lg: '4rem',
  xl: '6.625rem',
} as const;

export const radius = {
  sm: '0.25rem',
  md: '0.5rem',
  lg: '0.75rem',
} as const;

export const shadow = {
  button:
    '0 1px 2px rgba(13, 71, 161, 0.18), inset 0 1px 0 rgba(255, 255, 255, 0.35)',
  card: '0 8px 24px rgba(41, 44, 61, 0.06)',
  cardHover: '0 18px 48px rgba(41, 44, 61, 0.12)',
  glow: '0 0 24px rgba(103, 190, 255, 0.45)',
  glowLg: '0 0 80px rgba(103, 190, 255, 0.55)',
} as const;

export const easing = {
  out: [0.22, 1, 0.36, 1] as const,
  in: [0.7, 0, 0.84, 0] as const,
  inOut: [0.83, 0, 0.17, 1] as const,
  spring: [0.34, 1.56, 0.64, 1] as const,
} as const;

export const duration = {
  hover: 0.4,
  entrance: 0.6,
  hero: 0.9,
  scroll: 1.2,
} as const;
