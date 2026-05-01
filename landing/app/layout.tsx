import type { Metadata } from 'next';
import './globals.css';

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
    <html lang="en">
      <body className="font-sans antialiased">{children}</body>
    </html>
  );
}
