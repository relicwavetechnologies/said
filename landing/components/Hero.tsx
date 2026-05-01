import { TrustLogos } from './TrustLogos';

export function Hero() {
  return (
    <section className="relative isolate w-full overflow-hidden">
      <div className="water-bg absolute inset-0 -z-10" aria-hidden />

      <div className="mx-auto w-full max-w-[1280px] px-token-md pb-token-xl pt-[100px] md:pt-[140px]">
        <h1 className="text-[40px] font-normal leading-[1.06] tracking-[-0.02em] text-text sm:text-[48px] md:text-[56px] lg:text-[64px]">
          <span className="block whitespace-nowrap">We&rsquo;ve typed for 150 years.</span>
          <span className="block whitespace-nowrap">It&rsquo;s time to speak.</span>
        </h1>

        <p className="mt-8 max-w-[560px] text-[18px] leading-[1.55] text-text/85 md:text-[22px] md:leading-[1.5]">
          Aqua turns your voice into clear text in real time,
          <br />
          for everything from AI prompts to essays.
          <span
            aria-hidden
            className="pulse-dot ml-2 inline-block h-3 w-3 translate-y-[1px] rounded-full bg-accent align-middle shadow-glow"
          />
        </p>

        <div className="mt-16 flex flex-wrap items-center justify-between gap-y-6">
          <div className="flex items-center gap-3 text-[15px] text-text/85">
            <span>Hold</span>
            <kbd className="inline-flex h-9 min-w-[72px] items-center justify-center rounded-[8px] bg-white px-3.5 font-sans text-[13px] font-medium text-text shadow-[0_1px_0_rgba(0,0,0,0.04),0_4px_10px_rgba(41,44,61,0.08)] ring-1 ring-black/[0.06]">
              Space
            </kbd>
            <span>and try yourself</span>
          </div>

          <TrustLogos />
        </div>
      </div>
    </section>
  );
}
