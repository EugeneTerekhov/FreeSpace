import { Archive, Activity, Camera, Copy, Eraser, FolderTree, HardDrive, LayoutGrid, Radar, RefreshCw, ScrollText, Settings } from 'lucide-react'
import { Button, Collapse, Count, Icon, Meter, Spinner, Stage, States, Window, type Act } from '../kit'
import './hero.css'

// FreeSpace's hero: the drive map, a rescan, and the payoff — what can be
// freed. The product's own screen, framed wide and bleeding off the right and
// bottom. One cursor beat: press Rescan, watch the scan settle, and a
// recommendation bar opens under the map with the result.

type S = {
  scanning: boolean
  done: boolean
  progress: number
  scanned: number
  total: number
  rec: boolean
}

const TOTAL = 1_204_553
const NEXT = 1_204_611
const RECLAIMABLE = 312

const initial: S = { scanning: false, done: false, progress: 0, scanned: 0, total: TOTAL, rec: false }
const poster: S = { scanning: false, done: true, progress: 1, scanned: NEXT, total: NEXT, rec: true }

const STEPS: [progress: number, wait: number][] = [
  [0.14, 360],
  [0.33, 420],
  [0.55, 380],
  [0.74, 420],
  [0.9, 380],
  [1, 420],
]

const makeScript = (rest: { x: number; y: number }) => async (a: Act<S>) => {
  await a.wait(700)
  await a.move('rescan')
  await a.wait(280)
  await a.click()
  await a.set({ scanning: true, done: false, rec: false, progress: 0, scanned: 0 })
  await a.mark('scan')
  for (const [progress, wait] of STEPS) {
    await a.wait(wait)
    await a.set((s) => ({ progress, scanned: Math.round(NEXT * progress), total: s.total }))
  }
  await a.set({ scanning: false, done: true, scanned: NEXT, total: NEXT })
  await a.mark('done')
  await a.wait(240)
  await a.set({ rec: true })
  await a.mark('payoff')
  await a.wait(320)
  await a.move(rest)
  await a.wait(2800)
}

const NAV = [
  { icon: GaugeIcon, label: 'Диски' },
  { icon: Radar, label: 'Сканирование' },
  { icon: FolderTree, label: 'Дерево' },
  { icon: LayoutGrid, label: 'Карта', active: true },
  { icon: Copy, label: 'Дубликаты' },
  { icon: Eraser, label: 'Очистка' },
  { icon: Camera, label: 'Снимки' },
  { icon: ScrollText, label: 'Журнал' },
  { icon: Archive, label: 'Архивы' },
  { icon: Activity, label: 'Мониторинг' },
  { icon: Settings, label: 'Настройки' },
]

// A small hard-drive gauge, so the first nav row reads as the drive list
function GaugeIcon({ size = 16, strokeWidth = 1.6 }: { size?: number; strokeWidth?: number }) {
  return <HardDrive size={size} strokeWidth={strokeWidth} />
}

const MAP = [
  { name: 'Видео', size: '214 ГБ', tone: 'violet' },
  { name: 'Фото', size: '96 ГБ', tone: 'orange' },
  { name: 'Windows', size: '78 ГБ', tone: 'slate' },
  { name: 'Игры', size: '61 ГБ', tone: 'blue' },
  { name: 'Архивы', size: '42 ГБ', tone: 'amber' },
]

function Sidebar() {
  return (
    <div className="fs-side">
      <div className="fs-brand">
        <img className="fs-mark" src="/icon.png" alt="" />
        <span className="fs-wordmark">FreeSpace</span>
      </div>
      <nav className="fs-nav" aria-hidden>
        {NAV.map((item) => {
          const Glyph = item.icon
          return (
            <span key={item.label} className="fs-nav-item" data-active={item.active || undefined}>
              <Glyph size={16} strokeWidth={1.6} />
              <span className="fs-nav-label">{item.label}</span>
            </span>
          )
        })}
      </nav>
      <div className="fs-side-foot">
        <span className="fs-drive-dot" />
        <span className="fs-side-caption">C: · 288 ГБ свободно</span>
        <span className="fs-side-total">1 ТБ</span>
      </div>
    </div>
  )
}

function Treemap({ compact }: { compact?: boolean }) {
  return (
    <div className="fs-map" data-compact={compact || undefined} aria-hidden>
      {MAP.map((block) => (
        <div key={block.name} className="fs-block" data-kind={block.tone}>
          <div className="fs-block-fill">
            <div className="fs-block-head">
              <span className="fs-block-name">{block.name}</span>
              <span className="fs-block-size">{block.size}</span>
            </div>
            {block.name === 'Видео' && (
              <div className="fs-submap">
                <span className="fs-sub fs-sub--1" />
                <span className="fs-sub fs-sub--2" />
                <span className="fs-sub fs-sub--3" />
              </div>
            )}
            {block.name === 'Фото' && (
              <div className="fs-submap fs-submap--row">
                <span className="fs-sub fs-sub--1" />
                <span className="fs-sub fs-sub--2" />
              </div>
            )}
            {block.name === 'Windows' && (
              <div className="fs-submap fs-submap--stack">
                <span className="fs-sub fs-sub--1" />
                <span className="fs-sub fs-sub--2" />
              </div>
            )}
          </div>
        </div>
      ))}
    </div>
  )
}

function Scene({ s, variant }: { s: S; variant: 'wide' | 'medium' | 'compact' }) {
  const dense = variant !== 'wide'
  const title = variant === 'compact' ? 'Карта · C:' : 'Карта диска'
  return (
    <Window
      className="fs-window"
      data-variant={variant}
      title={title}
      sidebar={variant === 'wide' ? <Sidebar /> : undefined}
      sidebarWidth={208}
      toolbar={
        <Button
          variant="secondary"
          size="small"
          data-target="rescan"
          className="fs-rescan"
          icon={
            <States
              value={s.scanning ? 'scan' : 'idle'}
              states={{
                idle: <RefreshCw size={12} strokeWidth={1.7} />,
                scan: <Spinner size={12} />,
              }}
            />
          }
        >
          {s.scanning ? 'Сканирование' : 'Пересканировать'}
        </Button>
      }
    >
      <div className="fs-body" data-variant={variant}>
        <span className="fs-scanline" data-on={s.scanning || undefined}>
          <span style={{ transform: `scaleX(${s.progress})` }} />
        </span>

        <div className="fs-topline">
          <span className="fs-crumbs">
            <HardDrive size={15} strokeWidth={1.6} />
            <span className="fs-path">C:\</span>
            <span className="fs-chip">NVMe · NTFS</span>
          </span>
          <span className="fs-scan-status">
            <States
              align="start"
              value={s.scanning ? 'scan' : s.done ? 'done' : 'idle'}
              states={{
                idle: <span className="fs-status-text" />,
                scan: (
                  <span className="fs-status-text">
                    <Count value={s.scanned} duration={320} /> файлов
                  </span>
                ),
                done: (
                  <span className="fs-status-text fs-status-text--done">
                    <Icon name="check" size={14} /> Готово за 38 с
                  </span>
                ),
              }}
            />
          </span>
        </div>

        <div className="fs-usage">
          <span className="fs-usage-copy">
            <b>712 ГБ</b> занято из 1 ТБ
            <span className="fs-usage-sub">
              <span className="fs-sep">·</span>
              <Count value={s.total} />
              <span>объектов</span>
            </span>
          </span>
          <Meter value={0.712} className="fs-usage-meter" />
          <span className="fs-usage-free">288 ГБ свободно</span>
        </div>

        <Treemap compact={dense} />

        <Collapse open={s.rec} reveal="drop" from="below" className="fs-rec-slot">
          <div className="fs-rec" data-target="rec">
            <span className="fs-rec-mark">
              <Eraser size={16} strokeWidth={1.6} />
            </span>
            <span className="fs-rec-copy">
              <span className="fs-rec-title">
                <span>Можно освободить</span>
                <Count value={s.rec ? RECLAIMABLE : 0} duration={900} />
                <span>ГБ</span>
              </span>
              <span className="fs-rec-sub">12 групп дубликатов · 96 ГБ устаревшего · 4 старых снимка</span>
            </span>
            <Button variant="primary" size="small" className="fs-rec-action" icon={<Eraser size={12} strokeWidth={1.8} />}>
              {variant === 'compact' ? 'Открыть' : 'Открыть очистку'}
            </Button>
          </div>
        </Collapse>
      </div>
    </Window>
  )
}

const SIZES = {
  wide: { width: 1120, height: 560 },
  medium: { width: 660, height: 500 },
  compact: { width: 358, height: 350 },
} as const

const REST = {
  wide: { x: 104, y: 492 },
  medium: { x: 500, y: 39 },
  compact: { x: 300, y: 24 },
} as const

export function HeroScene({ variant }: { variant: 'wide' | 'medium' | 'compact' }) {
  const size = SIZES[variant]
  return (
    <Stage<S>
      width={size.width}
      height={size.height}
      initial={initial}
      poster={poster}
      script={makeScript(REST[variant])}
      play="once"
      theme="dark"
      fidelity="realistic"
      className={`fs-hero fs-hero--${variant}`}
      cursor={{ at: REST[variant], style: 'arrow' }}
      tokens={{
        '--ss-accent': '#ff8a3d',
        '--ss-accent-2': '#a06bff',
        '--ss-canvas': '#0d0b16',
        '--ss-surface': '#141120',
        '--ss-chrome': '#100e1a',
        '--ss-raised': '#1c1730',
        '--ss-line': 'rgba(255,255,255,0.075)',
        '--ss-line-faint': 'rgba(255,255,255,0.045)',
        '--ss-text': '#9d96ad',
        '--ss-text-strong': '#f2edf8',
        '--ss-text-faint': '#6c6479',
        '--ss-icon': '#9a92aa',
        '--ss-icon-faint': '#4b4458',
        '--ss-shadow-hero': '0 0 0 1px rgba(255,255,255,0.07), 0 48px 90px -30px rgba(0,0,0,0.9)',
        '--ss-radius-window': '14px',
        '--ss-font': '"Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif',
      }}
    >
      {(s) => <Scene s={s} variant={variant} />}
    </Stage>
  )
}
