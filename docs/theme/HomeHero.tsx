// Gukhanmun: Home hero override that animates the mixed-script-to-hangul pitch.
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

import type { ReactNode } from "react";
import { normalizeImagePath, useFrontmatter } from "@rspress/core/runtime";
import { Button, Link, renderHtmlOrText } from "@rspress/core/theme-original";
import { HeroConversion } from "./HeroConversion";

// Mirrors @rspress/core's stock HomeHero, but replaces the static `text`
// subtitle with the HeroConversion animation so the landing page demonstrates,
// rather than merely states, what Gukhanmun does. Everything else (brand title,
// tagline, action buttons, image) keeps the upstream markup and classes so the
// theme's hero styling continues to apply. The stock HomeLayout imports
// HomeHero from "@rspress/core/theme", which resolves to the custom theme
// module, so re-exporting this shadows the upstream component.
interface HomeHeroProps {
  beforeHeroActions?: ReactNode;
  afterHeroActions?: ReactNode;
  image?: ReactNode;
}

export function HomeHero({ beforeHeroActions, afterHeroActions, image }: HomeHeroProps) {
  const { frontmatter } = useFrontmatter();
  const hero = frontmatter?.hero;
  // The upstream component honours these slots, fed by HomeLayout; keep them.
  const hasImage = hero?.image !== undefined || image !== undefined;
  const imageSrc = typeof hero?.image?.src === "string"
    ? { light: hero.image.src, dark: hero.image.src }
    : hero?.image?.src || { light: "", dark: "" };

  return (
    <div className={`rp-home-hero${hasImage ? "" : " rp-home-hero--no-image"}`}>
      <div className="rp-home-hero__container">
        {hero?.badge && (typeof hero.badge === "string"
          ? <div className="rp-home-hero__badge">{hero.badge}</div>
          : hero.badge.link
          ? (
            <Link href={hero.badge.link} className="rp-home-hero__badge">
              {hero.badge.text}
            </Link>
          )
          : <div className="rp-home-hero__badge">{hero.badge.text}</div>)}
        <div className="rp-home-hero__content">
          <div className="rp-home-hero__title">
            <span
              className="rp-home-hero__title-brand"
              {...renderHtmlOrText(hero?.name)}
            >
            </span>
          </div>

          <HeroConversion />
        </div>
        <p
          className="rp-home-hero__tagline"
          {...renderHtmlOrText(hero?.tagline)}
        >
        </p>

        {beforeHeroActions}
        <div className="rp-home-hero__actions">
          {hero?.actions?.map((action) => (
            <Button
              type="a"
              key={action.link}
              href={action.link}
              theme={action.theme}
              className="rp-home-hero__action"
              {...renderHtmlOrText(action.text)}
            />
          ))}
        </div>
        {afterHeroActions}
      </div>
      {image
        ? <div className="rp-home-hero__image">{image}</div>
        : hero?.image
        ? (
          <div className="rp-home-hero__image">
            <img
              src={normalizeImagePath(imageSrc.light)}
              alt={hero?.image?.alt}
              width={375}
              height={375}
              className="rp-home-hero__image-img rp-home-hero__image-img--light"
            />
            <img
              src={normalizeImagePath(imageSrc.dark)}
              alt={hero?.image?.alt}
              width={375}
              height={375}
              className="rp-home-hero__image-img rp-home-hero__image-img--dark"
            />
          </div>
        )
        : null}
    </div>
  );
}
