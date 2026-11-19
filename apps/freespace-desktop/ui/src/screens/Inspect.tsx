import { useState } from 'react';

import { t } from '../i18n';
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
  const [tab, setTab] = useState<InspectTab>('metadata');

  return (
    <ScreenFrameTabs
      idBase="inspect"
      tabs={TABS.map((item) => ({ id: item.id, label: t(item.labelKey) }))}
      active={tab}
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
