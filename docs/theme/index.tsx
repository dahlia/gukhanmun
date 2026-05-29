// Gukhanmun: Custom Rspress theme with stable/canary environment switcher.
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

import "./index.css";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  copyToClipboard,
  IconArrowDown,
  IconLink,
  IconSuccess,
  Layout as OriginalLayout,
  SvgWrapper,
  useHoverGroup,
} from "@rspress/core/theme-original";
import { routePathToMdPath, useI18n, usePage, usePageData } from "@rspress/core/runtime";

export * from "@rspress/core/theme-original";
export { HomeHero } from "./HomeHero";

// Rspress's outline renders an "Open in chat" row whose dropdown is built
// solely from the "chatgpt"/"claude" view options, so "markdownLink" yields an
// empty menu, and there is no config flag to hide the row. It also ships a
// "Copy Markdown" button (LlmsCopyRow) but no way to grab the Markdown link
// itself. We override the open-in-chat slot to instead copy the current page's
// Markdown URL. The stock Outline imports LlmsOpenRow from
// "@rspress/core/theme", which resolves to this module, so this explicit
// export shadows the star re-export above.
export function LlmsOpenRow() {
  const t = useI18n();
  const { page } = usePageData();
  const pathname = routePathToMdPath(page.routePath);
  const [isFinished, setFinished] = useState(false);
  const timer = useRef<number | null>(null);

  const handleClick = useCallback(async () => {
    if (!pathname || typeof window === "undefined") return;
    const fullUrl = new URL(pathname, window.location.origin).toString();
    if (!(await copyToClipboard(fullUrl))) return;
    setFinished(true);
    if (timer.current) clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      setFinished(false);
      timer.current = null;
    }, 1500);
  }, [pathname]);

  if (!pathname) return null;

  return (
    <button className="rp-outline__action-row" onClick={handleClick}>
      <SvgWrapper icon={isFinished ? IconSuccess : IconLink} />
      <span>{t("copyMarkdownLinkText")}</span>
    </button>
  );
}

function EnvSwitcher() {
  const [env, setEnv] = useState<"stable" | "canary" | null>(null);
  const { page } = usePage();

  useEffect(() => {
    const host = window.location.hostname;
    if (host === "gukhanmun.org") setEnv("stable");
    else if (host === "canary.gukhanmun.org") setEnv("canary");
  }, []);

  const isCanary = env === "canary";
  const routePath = page.routePath ?? "/";

  const { hoverGroup, handleMouseEnter, handleMouseLeave } = useHoverGroup({
    items: env
      ? [
        {
          text: "Canary",
          link: isCanary ? "" : `https://canary.gukhanmun.org${routePath}`,
        },
        {
          text: "Stable",
          link: isCanary ? `https://gukhanmun.org${routePath}` : "",
        },
      ]
      : [],
    activeMatcher: (item) => item.text === (isCanary ? "Canary" : "Stable"),
  });

  if (!env) return null;

  return (
    <li
      className="rp-nav-menu__item env-switcher"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onClick={handleMouseEnter}
    >
      <div className="rp-nav-menu__item__container">
        {isCanary ? "Canary" : "Stable"}
        <SvgWrapper icon={IconArrowDown} className="rp-nav-menu__item__icon" />
      </div>
      {hoverGroup}
    </li>
  );
}

export function Layout() {
  return <OriginalLayout afterNavMenu={<EnvSwitcher />} />;
}
