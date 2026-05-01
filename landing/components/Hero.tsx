export function Hero() {
  return (
    <section className="relative isolate w-full overflow-hidden">
      <div className="water-bg absolute inset-0 -z-10" aria-hidden />

      <div className="mx-auto w-full max-w-[1280px] px-token-md pb-token-xl pt-[140px] md:pt-[180px]">
        <h1 className="text-[56px] font-normal leading-[1.04] tracking-[-0.015em] text-text sm:text-[72px] md:text-[88px] lg:text-[96px]">
          We&rsquo;ve typed for 150 years.
          <br />
          It&rsquo;s time to speak.
        </h1>

        <p className="mt-10 max-w-[640px] text-[22px] leading-[1.55] text-text/90 md:text-[26px] md:leading-[1.5]">
          Aqua turns your voice into clear text in real time,
          <br />
          for everything from AI prompts to essays.
          <span
            aria-hidden
            className="pulse-dot ml-2 inline-block h-3.5 w-3.5 translate-y-[1px] rounded-full bg-accent align-middle shadow-glow"
          />
        </p>

        <div className="mt-14 flex flex-wrap items-center gap-x-3 gap-y-2 text-[16px] text-text/80">
          <span>Hold</span>
          <kbd className="inline-flex h-10 min-w-[78px] items-center justify-center rounded-[10px] bg-white px-4 font-sans text-[14px] font-medium text-text shadow-[0_1px_0_rgba(0,0,0,0.04),0_4px_12px_rgba(41,44,61,0.08)] ring-1 ring-black/[0.06]">
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
      className="ml-1 inline-flex h-10 w-10 items-center justify-center rounded-[10px] bg-[#1d2030] text-white shadow-[0_4px_12px_rgba(29,32,48,0.18)]"
    >
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        <path
          d="M12 2.6 3.4 7.2v9.6L12 21.4l8.6-4.6V7.2L12 2.6Z"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinejoin="round"
        />
        <path
          d="m3.4 7.2 8.6 4.6 8.6-4.6M12 11.8V21.4"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
}
