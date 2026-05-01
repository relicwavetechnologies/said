import type { Metadata } from 'next';
import { Inter_Tight, Instrument_Serif } from 'next/font/google';
import './globals.css';
import { SmoothScrollProvider } from '@/components/providers/SmoothScrollProvider';
import { ScrollProgress } from '@/components/motion/ScrollProgress';

const interTight = Inter_Tight({
  subsets: ['latin'],
  weight: ['400', '500', '600'],
  variable: '--font-sans',
  display: 'swap',
});

const instrumentSerif = Instrument_Serif({
  subsets: ['latin'],
  weight: ['400'],
  style: ['normal', 'italic'],
  variable: '--font-display',
  display: 'swap',
});

export const metadata: Metadata = {
  title: 'Aqua — Speak. Don’t type.',
  description:
    'Aqua turns your voice into clear text in real time, for everything from AI prompts to essays.',
  metadataBase: new URL('https://withaqua.com'),
  openGraph: {
    title: 'Aqua — Speak. Don’t type.',
    description:
      'Aqua turns your voice into clear text in real time, for everything from AI prompts to essays.',
    type: 'website',
  },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${interTight.variable} ${instrumentSerif.variable}`}>
      <body className="font-sans antialiased">
        <div className="grain-overlay pointer-events-none fixed inset-0 z-[60] opacity-[0.035] mix-blend-multiply" aria-hidden />
        <ScrollProgress />
        <SmoothScrollProvider>{children}</SmoothScrollProvider>
      </body>
    </html>
  );
}
