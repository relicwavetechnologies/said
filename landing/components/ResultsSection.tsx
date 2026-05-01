export function ResultsSection() {
  return (
    <section className="relative w-full bg-black text-white">
      <div className="mx-auto w-full max-w-[1280px] px-token-md pb-token-xl pt-32">
        <div className="max-w-[640px]">
          <h2 className="text-[44px] font-normal leading-[1.04] tracking-[-0.02em] sm:text-[52px] md:text-[60px] lg:text-[64px]">
            Results you notice
            <br />
            immediately
          </h2>
          <p className="mt-6 max-w-[440px] text-[17px] leading-[1.55] text-white/55">
            Aqua helps developers ship faster, stay focused,
            <br />
            and spend less time on repetitive typing.
          </p>
        </div>

        <div className="mt-20 grid grid-cols-1 gap-x-16 gap-y-10 md:mt-28 md:grid-cols-2">
          <Stat value="6h 23m" label="Saved coding weekly" />
          <Stat value="230wpm" label="Write 5 times faster" />
        </div>
      </div>
    </section>
  );
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div className="flex flex-col">
      <div className="flex items-end justify-between gap-6 pb-6">
        <span className="text-[64px] font-normal leading-[1] tracking-[-0.03em] tabular-nums text-white sm:text-[80px] md:text-[96px] lg:text-[104px]">
          {value}
        </span>
        <span className="pb-3 text-[15px] text-white/55">{label}</span>
      </div>
      <div className="h-px w-full bg-white/15" />
    </div>
  );
}
