export const springs = {
  snappy: { type: "spring" as const, stiffness: 500, damping: 30, mass: 0.5 },
  default: { type: "spring" as const, stiffness: 300, damping: 26, mass: 0.8 },
  gentle: { type: "spring" as const, stiffness: 200, damping: 24, mass: 1 },
  drawer: { type: "spring" as const, stiffness: 380, damping: 32, mass: 1 },
  micro: { type: "spring" as const, stiffness: 600, damping: 35, mass: 0.3 },
  macEase: { type: "tween" as const, duration: 0.25, ease: [0.25, 0.1, 0.25, 1] as const },
  viewTransition: { type: "tween" as const, duration: 0.2, ease: [0.25, 0.1, 0.25, 1] as const },
  viewEnter: { type: "tween" as const, duration: 0.25, ease: [0.16, 1, 0.3, 1] as const },
};
