import type { ScreenId } from '../api/dto';
import { Analysis } from './Analysis';
import { Cleanup } from './Cleanup';
import { Duplicates } from './Duplicates';
import { FileOps } from './FileOps';
import { Inspect } from './Inspect';
import { Journal } from './Journal';
import { Monitor } from './Monitor';
import { Overview } from './Overview';
import { Settings } from './Settings';
import { Shred } from './Shred';
import { Tree } from './Tree';

/** Renders the active screen; the exhaustive switch fails to compile on a new id. */
export function ScreenRouter({ screen }: { screen: ScreenId }) {
  switch (screen) {
    case 'overview':
      return <Overview />;
    case 'tree':
      return <Tree />;
    case 'analysis':
      return <Analysis />;
    case 'duplicates':
      return <Duplicates />;
    case 'cleanup':
      return <Cleanup />;
    case 'journal':
      return <Journal />;
    case 'shred':
      return <Shred />;
    case 'monitor':
      return <Monitor />;
    case 'inspect':
      return <Inspect />;
    case 'fileops':
      return <FileOps />;
    case 'settings':
      return <Settings />;
    default: {
      // Unreachable while every `ScreenId` is handled above; it turns a newly
      // added screen into a compile-time error instead of a placeholder.
      const exhaustive: never = screen;
      return exhaustive;
    }
  }
}
