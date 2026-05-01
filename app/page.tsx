import { AnnouncementBar } from '@/components/AnnouncementBar';
import { Hero } from '@/components/Hero';
import { Navbar } from '@/components/Navbar';
import { SpeakSection } from '@/components/SpeakSection';
import { SpeedSection } from '@/components/SpeedSection';
import { FeaturesSection } from '@/components/FeaturesSection';
import { CodingSection } from '@/components/CodingSection';
import { ProductivitySection } from '@/components/ProductivitySection';
import { FaqSection } from '@/components/FaqSection';
import { ResultsSection } from '@/components/ResultsSection';

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
      <FeaturesSection />
      <CodingSection />
      <ProductivitySection />
      <FaqSection />
      <ResultsSection />
    </main>
  );
}
