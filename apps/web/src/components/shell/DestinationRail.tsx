import { Link, useLocation } from "@tanstack/react-router";
import {
  HomeIcon,
  SettingsIcon,
  type IconComponent,
} from "~/components/solar";
import { Tooltip, TooltipPopup, TooltipTrigger } from "~/components/ui/tooltip";
import { cn } from "~/lib/utils";

/**
 * The destination rail.
 *
 * Vendored from GOAT (`apps/web/src/components/sidebar/DestinationRail.tsx`,
 * MIT). GOAT's rail listed its own destinations - fleet, bot, pull requests,
 * hub - against a server that could hold many environments. LocalAide runs on
 * the user's own computer, so the rail lists the places the app can go.
 */
type RailItem = {
  readonly id: string;
  readonly label: string;
  readonly icon: IconComponent;
  readonly to: string;
};

const RAIL_ITEMS: RailItem[] = [
  { id: "home", label: "Chat", icon: HomeIcon, to: "/" },
];

const SETTINGS_ITEM: RailItem = {
  id: "settings",
  label: "Settings",
  icon: SettingsIcon,
  to: "/settings",
};

function railItemClassName(active: boolean) {
  return cn(
    "relative flex size-10 shrink-0 items-center justify-center rounded-lg text-sidebar-muted-foreground outline-hidden transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring [&_svg]:size-5",
    active && "bg-accent text-accent-foreground",
  );
}

export function DestinationRail() {
  const pathname = useLocation({ select: (location) => location.pathname });

  const isActive = (to: string) =>
    to === "/" ? pathname === "/" : pathname.startsWith(to);

  return (
    <nav
      aria-label="App"
      className="z-20 flex w-(--destination-rail-width) shrink-0 flex-col items-center border-r border-border bg-background"
      data-destination-rail=""
      data-glass-pane="rail"
    >
      <div className="flex w-full min-h-0 flex-1 flex-col items-center gap-1 overflow-y-auto pt-2 pb-3">
        {RAIL_ITEMS.map((item) => (
          <Tooltip key={item.id}>
            <TooltipTrigger
              render={
                <Link
                  to={item.to}
                  aria-label={item.label}
                  aria-current={isActive(item.to) ? "page" : undefined}
                  className={railItemClassName(isActive(item.to))}
                >
                  <item.icon aria-hidden />
                </Link>
              }
            />
            <TooltipPopup side="right">{item.label}</TooltipPopup>
          </Tooltip>
        ))}
        <div className="flex-1" />
        <Tooltip>
          <TooltipTrigger
            render={
              <Link
                to={SETTINGS_ITEM.to}
                aria-label={SETTINGS_ITEM.label}
                aria-current={isActive(SETTINGS_ITEM.to) ? "page" : undefined}
                className={railItemClassName(isActive(SETTINGS_ITEM.to))}
              >
                <SettingsIcon aria-hidden />
              </Link>
            }
          />
          <TooltipPopup side="right">{SETTINGS_ITEM.label}</TooltipPopup>
        </Tooltip>
      </div>
    </nav>
  );
}
