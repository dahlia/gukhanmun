// Gukhanmun: Animated hero showing mixed-script Korean converting to hangul.
// Copyright (C) 2026  Hong Minhee
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

import "./HeroConversion.css";
import { type CSSProperties, useEffect, useState } from "react";
import sentencesData from "./hero-sentences.json";

// A sentence is a list of segments. A plain segment renders verbatim; a pair
// segment renders its hanja form and animates into its hangul reading. The
// readings are the actual output of the bundled engine (hangul-only, ko-kr
// preset), so the animation mirrors what Gukhanmun really produces. The data
// lives in hero-sentences.json so the font-subsetting task (mise run
// docs-fonts) and this component share one source of truth for the glyphs.
type Segment = { t: string } | { h: string; k: string };

const SENTENCES = sentencesData as Segment[][];

// Timeline, in milliseconds: hold on the mixed-script form, convert one hanja
// word every STAGGER, hold on the hangul-only result, then fade to the next.
const HOLD_MIXED_MS = 1100;
const STAGGER_MS = 340;
const HOLD_HANGUL_MS = 2200;
const FADE_MS = 500;

function isPair(seg: Segment): seg is { h: string; k: string } {
  return "h" in seg;
}

// Track the user's motion preference so we can skip the animation entirely and
// render the finished hangul-only sentence for those who opt out.
function usePrefersReducedMotion(): boolean {
  const [reduce, setReduce] = useState(false);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReduce(mq.matches);
    update();
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  }, []);
  return reduce;
}

export function HeroConversion() {
  const reduceMotion = usePrefersReducedMotion();
  const [index, setIndex] = useState(0);
  // How many hanja words (left to right) have converted so far.
  const [converted, setConverted] = useState(0);
  const [visible, setVisible] = useState(true);

  const segments = SENTENCES[index];
  const pairCount = segments.filter(isPair).length;

  useEffect(() => {
    if (reduceMotion) {
      setVisible(true);
      setConverted(pairCount);
      return;
    }

    setVisible(true);
    setConverted(0);

    const timers: number[] = [];
    for (let i = 1; i <= pairCount; i++) {
      timers.push(
        window.setTimeout(() => setConverted(i), HOLD_MIXED_MS + i * STAGGER_MS),
      );
    }
    const settled = HOLD_MIXED_MS + pairCount * STAGGER_MS;
    timers.push(window.setTimeout(() => setVisible(false), settled + HOLD_HANGUL_MS));
    timers.push(
      window.setTimeout(
        () => setIndex((n) => (n + 1) % SENTENCES.length),
        settled + HOLD_HANGUL_MS + FADE_MS,
      ),
    );

    return () => {
      for (const t of timers) clearTimeout(t);
    };
  }, [index, pairCount, reduceMotion]);

  // The hanja layer stays in flow and defines each pair's box; the hangul layer
  // overlays it. Both forms have equal character counts, so the crossfade never
  // shifts the surrounding text. Decorative, so hide from assistive tech.
  let order = 0;
  return (
    <div
      className={`hero-conv${visible ? "" : " hero-conv--hidden"}`}
      aria-hidden="true"
    >
      <p className="hero-conv__line">
        {segments.map((seg, i) => {
          if (!isPair(seg)) {
            return (
              <span key={i} className="hero-conv__static">
                {seg.t}
              </span>
            );
          }
          order += 1;
          const done = converted >= order;
          return (
            <span
              key={i}
              className={`hero-conv__pair${done ? " is-converted" : ""}`}
              // Drives the cell width in CSS: hanja are full-width (1em),
              // hangul narrower, so the cell sizes to the syllable count.
              style={{ "--n": seg.h.length } as CSSProperties}
            >
              <span className="hero-conv__hanja">{seg.h}</span>
              <span className="hero-conv__hangul">{seg.k}</span>
            </span>
          );
        })}
      </p>
    </div>
  );
}
