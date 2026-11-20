import { useState } from 'react';

import { t } from '../i18n';
import { canUnlock, canUseLongPaths } from '../lib/capabilities';
import { usePlatformCapabilities } from '../stores/shellStore';
import { Archives } from './Archives';
import { LongPaths } from './LongPaths';
import { Metadata } from './Metadata';
import { ScreenFrameTabs } from './ScreenTabs';
import { Unlock } from './Unlock';

const TABS = [
  { id: 'metadata', labelKey: 'nav.metadata' },
  { id: 'archives', labelKey: 'nav.archives' },
  { id: 'unlock', labelKey: 'nav.unlock' },
  { id: 'long_paths', labelKey: 'nav.long_paths' },
] as const;

type InspectTab = (typeof TABS)[number]['id'];

/** Inspection: metadata, archives, unlock and long paths (T070). */
export function Inspect() {
  const capabilities = usePlatformCapabilities();
  // Restart Manager and verbatim long paths only exist on Windows; the tabs
  // that need them disappear on a backend without the capability.
  const tabs = TABS.filter((item) => {
    if (item.id === 'unlock') return canUnlock(capabilities);
    if (item.id === 'long_paths') return canUseLongPaths(capabilities);

    return true;
  });
  const [tab, setTab] = useState<InspectTab>('metadata');
  const active = tabs.some((item) => item.id === tab) ? tab : 'metadata';

  return (
    <ScreenFrameTabs
      idBase="inspect"
      tabs={tabs.map((item) => ({ id: item.id, label: t(item.labelKey) }))}
      active={active}
      onChange={(id) => {
        setTab(id as InspectTab);
      }}
    >
      {(id) => {
        if (id === 'metadata') return <Metadata />;
        if (id === 'archives') return <Archives />;
        if (id === 'unlock') return <Unlock />;
        if (id === 'long_paths') return <LongPaths />;
        return null;
      }}
    </ScreenFrameTabs>
  );
}
