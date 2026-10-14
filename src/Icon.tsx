import type { SVGProps } from "react";
export type IconName = "packs" | "activity" | "settings" | "plus" | "arrow" | "check" | "more" | "folder" | "close" | "download";
const paths: Record<IconName, string[]> = {
 packs: ["M3 7h18v14H3z", "M7 3h10v4", "M9 12h6"],
 activity: ["M3 12h4l3-7 4 14 3-7h4"],
 settings: ["M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8", "M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1z"],
 plus: ["M12 5v14", "M5 12h14"], arrow: ["M15 5l-7 7 7 7"],
 check: ["M5 12l4 4L19 6"], more: ["M5 12h.01", "M12 12h.01", "M19 12h.01"],
 folder: ["M3 7V4h6l2 3h10v13H3z"], close: ["M6 6l12 12", "M18 6L6 18"],
 download: ["M12 3v12", "M7 10l5 5 5-5", "M4 17v4h16v-4"]
};
export function Icon({ name, ...props }: SVGProps<SVGSVGElement> & { name: IconName }) {
 return <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" {...props}>
   {paths[name].map((d) => <path key={d} d={d} />)}
 </svg>;
}
