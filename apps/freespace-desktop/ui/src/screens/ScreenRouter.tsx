import type { ScreenId } from '../api/dto';
import { About } from './About';
import { Archives } from './Archives';
import { Cleanup } from './Cleanup';
import { Dashboard } from './Dashboard';
import { Duplicates } from './Duplicates';
import { Elevation } from './Elevation';
import { Export } from './Export';
import { Journal } from './Journal';
import { LongPaths } from './LongPaths';
import { Metadata } from './Metadata';
import { Monitor } from './Monitor';
import { Owners } from './Owners';
import { Permissions } from './Permissions';
import { Rename } from './Rename';
import { Scan } from './Scan';
import { Settings } from './Settings';
import { Shred } from './Shred';
import { SizeDetails } from './SizeDetails';
import { Snapshots } from './Snapshots';
import { TopFiles } from './TopFiles';
import { Tree } from './Tree';
import { Treemap } from './Treemap';
import { Types } from './Types';
import { Unlock } from './Unlock';

/** Renders the active screen; the exhaustive switch fails to compile on a new id. */
export function ScreenRouter({ screen }: { screen: ScreenId }) {
  switch (screen) {
    case 'dashboard':
      return <Dashboard />;
    case 'scan':
      return <Scan />;
    case 'tree':
      return <Tree />;
    case 'treemap':
      return <Treemap />;
    case 'top_files':
      return <TopFiles />;
    case 'types':
      return <Types />;
    case 'duplicates':
      return <Duplicates />;
    case 'cleanup':
      return <Cleanup />;
    case 'snapshots':
      return <Snapshots />;
    case 'journal':
      return <Journal />;
    case 'size_details':
      return <SizeDetails />;
    case 'long_paths':
      return <LongPaths />;
    case 'rename':
      return <Rename />;
    case 'permissions':
      return <Permissions />;
    case 'metadata':
      return <Metadata />;
    case 'owners':
      return <Owners />;
    case 'archives':
      return <Archives />;
    case 'export':
      return <Export />;
    case 'unlock':
      return <Unlock />;
    case 'shred':
      return <Shred />;
    case 'monitor':
      return <Monitor />;
    case 'elevation':
      return <Elevation />;
    case 'settings':
      return <Settings />;
    case 'about':
      return <About />;
    default: {
      // Unreachable while every `ScreenId` is handled above; it turns a newly
      // added screen into a compile-time error instead of a placeholder.
      const exhaustive: never = screen;
      return exhaustive;
    }
  }
}
