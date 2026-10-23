// Gukhanmun: Shared funding links and localized home sponsorship section.
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

import { useFrontmatter, useLang } from "@rspress/core/runtime";
import { Link } from "@rspress/core/theme-original";
import "./Sponsorship.css";

/** The two public funding channels, presented with equal prominence. */
export function FundingChannels() {
  return (
    <div className="funding-channels">
      <a href="https://github.com/sponsors/dahlia" className="funding-channel">
        <span>GitHub Sponsors</span>
        <span aria-hidden="true">↗</span>
      </a>
      <a
        href="https://opencollective.com/gukhanmun"
        className="funding-channel"
      >
        <span>Open Collective</span>
        <span aria-hidden="true">↗</span>
      </a>
    </div>
  );
}

interface HomeSponsorshipContent {
  title: string;
  description: string;
  details: string;
  acknowledgement: string;
}

/** Home-page copy comes from frontmatter so ko-KR is generated with the page. */
export function HomeSponsorship() {
  const { frontmatter } = useFrontmatter();
  const lang = useLang();
  const sponsorshipPath = lang === "en" ? "/sponsorship" : `/${lang}/sponsorship`;
  const content = frontmatter.sponsorship as HomeSponsorshipContent | undefined;
  if (!content) return null;

  return (
    <section
      className="home-sponsorship"
      aria-labelledby="home-sponsorship-title"
    >
      <div className="home-sponsorship__body">
        <h2 id="home-sponsorship-title">{content.title}</h2>
        <p>{content.description}</p>
        <Link href={sponsorshipPath} className="home-sponsorship__details">
          {content.details}
          <span aria-hidden="true">→</span>
        </Link>
      </div>
      <div className="home-sponsorship__actions">
        <FundingChannels />
        <p className="home-sponsorship__acknowledgement">
          {content.acknowledgement}
        </p>
      </div>
    </section>
  );
}
