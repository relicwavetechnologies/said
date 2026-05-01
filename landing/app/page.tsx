import { AnnouncementBar } from '@/components/AnnouncementBar';
import { Hero } from '@/components/Hero';
import { Navbar } from '@/components/Navbar';
import { SpeakSection } from '@/components/SpeakSection';
import { SpeedSection } from '@/components/SpeedSection';

export default function HomePage() {
  return (
    <main className="min-h-screen bg-background">
      <AnnouncementBar
        message="Now live on iOS"
        ctaLabel="Download"
        ctaHref="/ios"
      />
      <Navbar />
      <Hero />
      <SpeakSection />
      <SpeedSection />
    </main>
  );
}
