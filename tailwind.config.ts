import type { Config } from 'tailwindcss';
import { colors, fontFamily, fontSize, radius, shadow, spacing } from './lib/tokens';

const config: Config = {
  content: ['./app/**/*.{ts,tsx}', './components/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        accent: colors.accent,
        'accent-text': colors.accentText,
        'accent-soft': '#c3e3ff',
        'accent-deep': '#1f7fd8',
        background: colors.background,
        ink: colors.ink,
        text: colors.text,
        muted: colors.muted,
      },
      fontFamily: {
        sans: fontFamily.sans as unknown as string[],
        display: ['var(--font-display)', 'Instrument Serif', 'Georgia', 'serif'],
      },
      fontSize: fontSize as unknown as Record<string, [string, { lineHeight: string; letterSpacing?: string }]>,
      spacing: {
        'token-xs': spacing.xs,
        'token-sm': spacing.sm,
        'token-md': spacing.md,
        'token-lg': spacing.lg,
        'token-xl': spacing.xl,
      },
      borderRadius: {
        'token-sm': radius.sm,
        'token-md': radius.md,
        'token-lg': radius.lg,
      },
      boxShadow: {
        button: shadow.button,
        card: shadow.card,
        'card-hover': shadow.cardHover,
        glow: shadow.glow,
        'glow-lg': shadow.glowLg,
      },
      backdropBlur: {
        xs: '2px',
      },
      keyframes: {
        'mesh-drift': {
          '0%, 100%': { transform: 'translate3d(0,0,0) scale(1)' },
          '50%': { transform: 'translate3d(40px,-30px,0) scale(1.05)' },
        },
        marquee: {
          '0%': { transform: 'translateX(0%)' },
          '100%': { transform: 'translateX(-50%)' },
        },
      },
      animation: {
        'mesh-drift': 'mesh-drift 18s ease-in-out infinite',
        marquee: 'marquee 28s linear infinite',
      },
    },
  },
  plugins: [],
};

export default config;
