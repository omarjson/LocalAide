  interface Props {
    name: IconName;
    class?: string;
    /** Explicit pixel size; Tailwind classes still win when present. */
    size?: number;
    /** Stroke weight on the 24px grid. */
    strokeWidth?: number;
  }

  // Typed icon registry for all used icons
  const ICONS: Record<IconName, { size: string; strokeWidth: number }> = {
    chat: { size: "20px", strokeWidth: 2 },
    search: { size: "20px", strokeWidth: 2 },
    memory: { size: "20px", strokeWidth: 2 },
    skills: { size: "20px", strokeWidth: 2 },
    models: { size: "20px", strokeWidth: 2 },
    settings: { size: "20px", strokeWidth: 2 },
    usage: { size: "20px", strokeWidth: 2 },
    automations: { size: "20px", strokeWidth: 2 },
    // New icons from requirement 7
    compare: { size: "20px", strokeWidth: 2 },
    target: { size: "20px", strokeWidth: 2 },
    calendar: { size: "20px", strokeWidth: 2 },
    puzzle: { size: "20px", strokeWidth: 2 },
    route: { size: "20px", strokeWidth: 2 },
    shield_check: { size: "20px", strokeWidth: 2 },
    keychain: { size: "20px", strokeWidth: 2 },
    stethoscope: { size: "20px", strokeWidth: 2 },
    panel: { size: "20px", strokeWidth: 2 },
    history: { size: "20px", strokeWidth: 2 },
    merge: { size: "20px", strokeWidth: 2 },
  };

  function getIconProps(name: IconName) {
    return ICONS[name] ?? { size: "20px", strokeWidth: 2 };
  }
