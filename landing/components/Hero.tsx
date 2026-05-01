export function Hero() {
  return (
    <section className="relative isolate w-full overflow-hidden">
      <div className="water-bg absolute inset-0 -z-10" aria-hidden />

      <div className="mx-auto w-full max-w-[1280px] px-token-md pb-token-xl pt-[120px]">
        <h1 className="max-w-[14ch] text-[64px] font-normal leading-[1.05] tracking-[-0.01em] text-text md:text-[88px] md:leading-[1.02]">
          We&rsquo;ve typed for 150 years.
          <br />
          It&rsquo;s time to speak.
        </h1>

        <p className="mt-token-md flex max-w-[44ch] items-center gap-2 text-[20px] leading-[1.6] text-text/85 md:text-[24px]">
          <span>
            Aqua turns your voice into clear text in real time,
            <br className="hidden md:block" />
            for everything from AI prompts to essays.
          </span>
          <span
            aria-hidden
            className="pulse-dot inline-block h-3 w-3 shrink-0 rounded-full bg-accent shadow-glow"
          />
        </p>

        <div className="mt-token-lg flex items-center gap-3 text-[15px] text-text/80">
          <span>Hold</span>
          <kbd className="inline-flex h-9 min-w-[68px] items-center justify-center rounded-token-md border border-black/10 bg-white px-3 font-mono text-[13px] text-text shadow-card">
            Space
          </kbd>
          <span>and speak to refactor a function</span>
          <CubeBadge />
        </div>
      </div>
    </section>
  );
}

function CubeBadge() {
  return (
    <span
      aria-hidden
      className="ml-1 inline-flex h-9 w-9 items-center justify-center rounded-token-md bg-[#1f2230] text-white shadow-card"
    >
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        <path
          d="M12 2.5 3 7v10l9 4.5 9-4.5V7l-9-4.5Z"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinejoin="round"
        />
        <path
          d="m3 7 9 4.5L21 7M12 11.5V21.5"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
}
