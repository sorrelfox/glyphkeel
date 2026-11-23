// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import {
  Book,
  Briefcase,
  Bug,
  Building,
  Calendar,
  Camera,
  ChartLine,
  Code,
  Coffee,
  Compass,
  Database,
  FlaskConical,
  Flag,
  Folder,
  Gamepad,
  Globe,
  GraduationCap,
  Hammer,
  Heart,
  House,
  Layers,
  Leaf,
  Lightbulb,
  Mountain,
  Music,
  Package,
  Palette,
  Plane,
  Puzzle,
  Rocket,
  Shield,
  ShoppingCart,
  Sparkles,
  Star,
  Target,
  Terminal,
  Users,
  Wallet,
  Wrench,
  Zap,
  type LucideIcon,
} from "lucide-react";

import { classes } from "../classes";
import { PROJECT_ICONS } from "../data/catalog";
import { type Hue, type ProjectIcon } from "../generated/ops";

/** The icon of each project icon name; the names are the icon family's own. */
const ICONS: Readonly<Record<ProjectIcon, LucideIcon>> = {
  folder: Folder,
  star: Star,
  bug: Bug,
  rocket: Rocket,
  book: Book,
  house: House,
  briefcase: Briefcase,
  code: Code,
  terminal: Terminal,
  globe: Globe,
  heart: Heart,
  flag: Flag,
  zap: Zap,
  target: Target,
  lightbulb: Lightbulb,
  wrench: Wrench,
  hammer: Hammer,
  palette: Palette,
  music: Music,
  camera: Camera,
  "graduation-cap": GraduationCap,
  "shopping-cart": ShoppingCart,
  wallet: Wallet,
  "chart-line": ChartLine,
  calendar: Calendar,
  users: Users,
  building: Building,
  plane: Plane,
  leaf: Leaf,
  coffee: Coffee,
  gamepad: Gamepad,
  puzzle: Puzzle,
  shield: Shield,
  "flask-conical": FlaskConical,
  layers: Layers,
  package: Package,
  compass: Compass,
  mountain: Mountain,
  sparkles: Sparkles,
  database: Database,
};

interface ProjectMarkProps {
  hue: Hue;
  /** Without one, the project's mark is its coloured circle. */
  icon?: ProjectIcon | undefined;
  /** Spoken name; without one the mark is decoration. */
  label?: string | undefined;
  className?: string | undefined;
}

/**
 * A project's mark wherever the project is named: its icon drawn in its
 * colour, or the coloured circle when it has no icon.
 */
export function ProjectMark({ hue, icon, label, className }: ProjectMarkProps) {
  const spoken = label === undefined ? { "aria-hidden": true as const } : { role: "img", "aria-label": label };
  const Icon = icon === undefined ? undefined : ICONS[icon];
  if (Icon === undefined) {
    return <span className={classes("dot", `hue-${hue}`, className)} data-mark="circle" {...spoken} />;
  }
  return <Icon className={classes("ic s mark", `hue-${hue}`, className)} data-mark={icon} {...spoken} />;
}

export { PROJECT_ICONS };

/** One icon of the picker, without a colour of its own. */
export function ProjectIconGlyph({ icon, className }: { icon: ProjectIcon; className?: string | undefined }) {
  const Icon = ICONS[icon];
  return <Icon className={classes("ic", className)} aria-hidden="true" />;
}
