import { useState } from 'react';

import { t } from '../i18n';
import { ExportAction } from '../components/ExportAction';
import { Owners } from './Owners';
import { ScreenFrameTabs } from './ScreenTabs';
import { TopFiles } from './TopFiles';
import { Types } from './Types';

const TABS = [
  { id: 'top', labelKey: 'nav.top' },
  { id: 'types', labelKey: 'nav.types' },
  { id: 'owners', labelKey: 'nav.owners' },
] as const;

type AnalysisTab = (typeof TABS)[number]['id'];

/** Analysis: largest files, file types and owners as three tabs (T040). */
export function Analysis() {
  const [tab, setTab] = useState<AnalysisTab>('top');

  return (
    <ScreenFrameTabs
      idBase="analysis"
      tabs={TABS.map((item) => ({ id: item.id, label: t(item.labelKey) }))}
      active={tab}
      onChange={(id) => {
        setTab(id as AnalysisTab);
      }}
      actions={<ExportAction sections={['top', 'types', 'owners']} />}
    >
      {(id) => {
        if (id === 'top') return <TopFiles />;
        if (id === 'types') return <Types />;
        if (id === 'owners') return <Owners />;
        return null;
      }}
    </ScreenFrameTabs>
  );
}
