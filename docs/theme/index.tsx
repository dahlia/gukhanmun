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
import { useEffect, useState } from "react";
import {
  IconArrowDown,
  Layout as OriginalLayout,
  SvgWrapper,
  useHoverGroup,
} from "@rspress/core/theme-original";
import { usePage } from "@rspress/core/runtime";

export * from "@rspress/core/theme-original";

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
