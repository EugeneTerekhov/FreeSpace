import { useState } from 'react';

import { t } from '../i18n';
import { Permissions } from './Permissions';
import { Rename } from './Rename';
import { ScreenFrameTabs } from './ScreenTabs';

const TABS = [
  { id: 'rename', labelKey: 'nav.rename' },
  { id: 'permissions', labelKey: 'nav.permissions' },
] as const;

type FileOpsTab = (typeof TABS)[number]['id'];

/** File operations: rename and permissions (T071). */
export function FileOps() {
  const [tab, setTab] = useState<FileOpsTab>('rename');

  return (
    <ScreenFrameTabs
      idBase="fileops"
      tabs={TABS.map((item) => ({ id: item.id, label: t(item.labelKey) }))}
      active={tab}
      onChange={(id) => {
        setTab(id as FileOpsTab);
      }}
    >
      {(id) => {
        if (id === 'rename') return <Rename />;
        if (id === 'permissions') return <Permissions />;
        return null;
      }}
    </ScreenFrameTabs>
  );
}
