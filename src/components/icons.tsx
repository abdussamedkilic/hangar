import type { ReactNode } from "react";

function Icon({ children }: { children: ReactNode }) {
  return (
    <svg className="ico" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
      {children}
    </svg>
  );
}

export const FilterIcon = () => <Icon><path d="M3 5h18l-7 8v6l-4-2v-4z" /></Icon>;
export const InstalledIcon = () => <Icon><circle cx="12" cy="12" r="9" /><path d="m8 12.5 3 3 5-6" /></Icon>;
export const FilterPlusIcon = () => <Icon><path d="M3 5h15l-6 7v6l-4-2v-4zM19 14v6M16 17h6" /></Icon>;
export const AddIcon = () => <Icon><path d="M12 5v14M5 12h14" /></Icon>;
export const RefreshIcon = () => <Icon><path d="M20 11a8 8 0 0 0-14.5-4M4 5v3.5h3.5M4 13a8 8 0 0 0 14.5 4M20 19v-3.5h-3.5" /></Icon>;
export const SettingsIcon = () => <Icon><circle cx="12" cy="12" r="3" /><path d="M12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8" /></Icon>;
export const CollapseIcon = () => <Icon><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M9 4v16M16 10l-2 2 2 2" /></Icon>;
export const ExpandIcon = () => <Icon><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M9 4v16M14 10l2 2-2 2" /></Icon>;
