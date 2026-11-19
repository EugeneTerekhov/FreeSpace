import type { ReactNode } from 'react';

import { Screen, TabPanel, Tabs, type TabItem } from './ScreenFrame';
import styles from './screen.module.css';

export interface ScreenFrameTabsProps {
  /** Stable prefix for tab/panel ids. */
  idBase: string;
  tabs: readonly TabItem[];
  active: string;
  onChange: (id: string) => void;
  /** One line of context for the screen head. */
  context?: ReactNode;
  actions?: ReactNode;
  /** Renders the content of one tab; every panel stays mounted. */
  children: (id: string) => ReactNode;
}

/**
 * Screen body made of tabs.
 *
 * Every panel stays mounted (`hidden`), so switching tabs keeps filters,
 * sorting, selection and scroll position (contract §4.2).
 */
export function ScreenFrameTabs({
  idBase,
  tabs,
  active,
  onChange,
  context,
  actions,
  children,
}: ScreenFrameTabsProps) {
  return (
    <Screen context={context}>
      <div className={styles.tabsRow}>
        <Tabs idBase={idBase} items={tabs} active={active} onChange={onChange} />
        {actions !== undefined ? <div className={styles.tabsActions}>{actions}</div> : null}
      </div>
      {tabs.map((tab) => (
        <TabPanel key={tab.id} idBase={idBase} id={tab.id} active={tab.id === active}>
          {children(tab.id)}
        </TabPanel>
      ))}
    </Screen>
  );
}
