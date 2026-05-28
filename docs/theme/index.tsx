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
