import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { Check, Copy, File, ShieldCheck, Timer, Trash2 } from 'lucide-react'
import {
  Badge,
  Button,
  Chart,
  Count,
  Row,
  Spinner,
  Stage,
  States,
  Text,
  useReducedMotion,
  type Act,
} from '../kit'
import './bento.css'

// The features grid: one hero tile (a duplicate group being confirmed), a
// number, a diagram, a component and a statement. Nothing repeats a window:
// the page's hero already carries the app; these are fragments of it.

type Beat = { on: boolean }
const before: Beat = { on: false }
const after: Beat = { on: true }

const LEAD = 420
const STEP = 120
const beat = (i: number) => async (a: Act<Beat>) => {
  await a.wait(LEAD + i * STEP)
  await a.set({ on: true })
}

function usePictureWidth() {
  const ref = useRef<HTMLDivElement>(null)
  const [width, setWidth] = useState(340)
  useLayoutEffect(() => {
    const node = ref.current
    if (!node) return
    const measure = () => setWidth(Math.max(1, node.getBoundingClientRect().width))
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(node)
    return () => observer.disconnect()
  }, [])
  return [ref, width] as const
}

function useInView<T extends Element>(delay: number) {
  const ref = useRef<T>(null)
  const [seen, setSeen] = useState(false)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    let timer = 0
    const io = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) return
      io.disconnect()
      timer = window.setTimeout(() => setSeen(true), delay)
    }, { threshold: 0.35 })
    io.observe(el)
    return () => {
      io.disconnect()
      clearTimeout(timer)
    }
  }, [delay])
  return [ref, seen] as const
}

function Tile({
  kind,
  title,
  children,
  picture,
}: {
  kind: string
  title?: ReactNode
  children?: ReactNode
  picture?: ReactNode
}) {
  return (
    <section className={`fs-tile fs-tile--${kind}`}>
      {title && (
        <p className="fs-tile-copy">
          {title} {children}
        </p>
      )}
      {picture}
    </section>
  )
}

/* ---------------- Duplicates (hero UI) ---------------- */

const FILES = [
  { name: 'prod-2026-03-14.sql', size: '6,1 ГБ', keep: true },
  { name: 'prod-2026-03-14 (1).sql', size: '6,1 ГБ' },
  { name: 'prod-2026-03-14 (2).sql', size: '6,1 ГБ' },
]

function Duplicates() {
  const [picture, width] = usePictureWidth()
  const narrow = width < 500
  return (
    <Tile
      kind="dupes"
      title={<b>Дубликаты, проверенные побайтно.</b>}
      picture={
        <div ref={picture} className="fs-tile-picture">
          <Stage<Beat>
            width={width}
            height={narrow ? 300 : 280}
            initial={before}
            poster={after}
            play="once"
            script={beat(0)}
            theme="dark"
            fidelity="realistic"
            className="fs-tile-stage"
            tokens={{ '--ss-accent': '#ff8a3d', '--ss-radius-window': '14px' }}
          >
            {(s) => (
              <div className="fs-dup" data-narrow={narrow || undefined}>
                <div className="fs-dup-head">
                  <span className="fs-dup-icon">
                    <Copy size={15} strokeWidth={1.6} />
                  </span>
                  <span className="fs-dup-title">Дампы базы</span>
                  <States
                    className="fs-dup-badge"
                    align="end"
                    value={s.on ? 'done' : 'check'}
                    states={{
                      check: (
                        <Badge tone="warning" icon={<Spinner size={11} />}>
                          Проверяем
                        </Badge>
                      ),
                      done: (
                        <Badge tone="success" icon={<Check size={12} strokeWidth={2.2} />}>
                          Идентичны
                        </Badge>
                      ),
                    }}
                  />
                </div>
                <div className="fs-dup-rows">
                  {FILES.map((file) => (
                    <Row
                      key={file.name}
                      className="fs-dup-row"
                      pad={8}
                      lead={
                        <span className="fs-file" data-keep={file.keep || undefined}>
                          <File size={14} strokeWidth={1.6} />
                        </span>
                      }
                      title={<Text className="fs-dup-name">{file.name}</Text>}
                      meta={<Text className="fs-dup-size">{file.size}</Text>}
                    >
                      {file.keep ? (
                        <span className="fs-keep">
                          <Check size={12} strokeWidth={2.2} />
                          Оставить
                        </span>
                      ) : (
                        <span className="fs-drop" data-on={s.on || undefined}>
                          <Trash2 size={14} strokeWidth={1.7} />
                        </span>
                      )}
                    </Row>
                  ))}
                </div>
                <div className="fs-dup-foot">
                  <span className="fs-dup-foot-label">Освободится</span>
                  <span className="fs-dup-total">
                    <Count value={s.on ? 12.3 : 0} duration={900} format={(n) => n.toFixed(1).replace('.', ',')} />
                    <span className="fs-dup-unit">ГБ</span>
                  </span>
                  <Button variant="primary" size="small" icon={<Trash2 size={12} strokeWidth={1.9} />} className="fs-dup-action">
                    {narrow ? 'Удалить' : 'Удалить 2 копии'}
                  </Button>
                </div>
              </div>
            )}
          </Stage>
        </div>
      }
    >
      Похожие имена не считаются: совпадение подтверждает содержимое.
    </Tile>
  )
}

/* ---------------- Scan speed (stat) ---------------- */

function ScanStat() {
  const reduced = useReducedMotion()
  const [ref, seen] = useInView<HTMLElement>(LEAD + STEP)
  return (
    <section ref={ref} className="fs-tile fs-tile--stat">
      <span className="fs-stat-top">
        <Timer size={15} strokeWidth={1.6} />
        Скорость сканирования
      </span>
      <div className="fs-stat-body">
        <span className="fs-stat-lead">8 ТБ за</span>
        <span className="fs-stat-number">
          <Count value={seen || reduced ? 38 : 0} duration={1000} />
          <span className="fs-stat-unit">с</span>
        </span>
        <span className="fs-stat-label">MFT и USN-журнал вместо обхода каждой папки.</span>
      </div>
    </section>
  )
}

/* ---------------- Snapshots (diagram) ---------------- */

const CHART = [46, 52, 49, 58, 55, 66, 61, 72, 68, 79]

function Snapshots() {
  const [picture, width] = usePictureWidth()
  const chartWidth = Math.max(160, width - 84)
  return (
    <Tile
      kind="snap"
      title={<b>Снимки, а не догадки.</b>}
      picture={
        <div ref={picture} className="fs-tile-picture">
          <Stage<Beat>
            width={width}
            height={240}
            initial={before}
            poster={after}
            play="once"
            script={beat(2)}
            theme="dark"
            className="fs-tile-stage"
            tokens={{ '--ss-accent': '#ff8a3d', '--ss-accent-2': '#a06bff' }}
          >
            {(s) => (
              <div className="fs-snap" data-on={s.on || undefined}>
                <div className="fs-snap-head">
                  <span className="fs-snap-title">C: · снимок диска</span>
                  <span className="fs-snap-range">12 марта → 16 ноября</span>
                </div>
                <Chart data={CHART} width={chartWidth} height={120} show={s.on} area dot className="fs-snap-chart" />
                <div className="fs-snap-foot">
                  <span className="fs-snap-label">Свободно прибавилось</span>
                  <span className="fs-snap-value">
                    <span className="fs-snap-plus">+</span>
                    <Count value={s.on ? 61.7 : 0} duration={900} format={(n) => n.toFixed(1).replace('.', ',')} />
                    <span className="fs-snap-unit">ГБ</span>
                  </span>
                </div>
              </div>
            )}
          </Stage>
        </div>
      }
    >
      Сравните два дня и увидите, что выросло.
    </Tile>
  )
}

/* ---------------- CLI (component) ---------------- */

function CliTile() {
  const [picture, width] = usePictureWidth()
  return (
    <Tile
      kind="cli"
      title={<b>Тот же движок из PowerShell.</b>}
      picture={
        <div ref={picture} className="fs-tile-picture">
          <Stage<Beat>
            width={width}
            height={240}
            initial={before}
            poster={after}
            play="once"
            script={beat(3)}
            theme="dark"
            className="fs-tile-stage"
            tokens={{ '--ss-accent': '#ff8a3d' }}
          >
            {(s) => (
              <div className="fs-cli">
                <div className="fs-cli-body">
                  <div className="fs-cli-line">
                    <span className="fs-cli-prompt">PS C:\&gt;</span>{' '}
                    <span className="fs-cli-cmd">freespace duplicates C:\ --json</span>
                  </div>
                  <div className="fs-cli-line fs-cli-out" data-on={s.on || undefined} style={{ transitionDelay: '0ms' }}>
                    <span className="fs-json-p">{'{'}</span>
                  </div>
                  <div className="fs-cli-line fs-cli-out" data-on={s.on || undefined} style={{ transitionDelay: '90ms' }}>
                    <span className="fs-json-k">"reclaimable_bytes"</span>
                    <span className="fs-json-p">: </span>
                    <span className="fs-json-n">13207024435</span>
                    <span className="fs-json-p">,</span>
                  </div>
                  <div className="fs-cli-line fs-cli-out" data-on={s.on || undefined} style={{ transitionDelay: '180ms' }}>
                    <span className="fs-json-k">"groups"</span>
                    <span className="fs-json-p">: </span>
                    <span className="fs-json-n">12</span>
                    <span className="fs-json-p">,</span>
                  </div>
                  <div className="fs-cli-line fs-cli-out" data-on={s.on || undefined} style={{ transitionDelay: '270ms' }}>
                    <span className="fs-json-k">"verified"</span>
                    <span className="fs-json-p">: </span>
                    <span className="fs-json-s">"sha256"</span>
                  </div>
                  <div className="fs-cli-line fs-cli-out" data-on={s.on || undefined} style={{ transitionDelay: '360ms' }}>
                    <span className="fs-json-p">{'}'}</span>
                  </div>
                  <div className="fs-cli-line fs-cli-out" data-on={s.on || undefined} style={{ transitionDelay: '450ms' }}>
                    <span className="fs-cli-prompt">PS C:\&gt;</span>{' '}
                    <span className="fs-cli-cmd">$LASTEXITCODE</span>
                  </div>
                  <div className="fs-cli-line fs-cli-out" data-on={s.on || undefined} style={{ transitionDelay: '540ms' }}>
                    <span className="fs-cli-ok">0</span>
                  </div>
                </div>
              </div>
            )}
          </Stage>
        </div>
      }
    >
      JSON, блокировка параллельных запусков и коды возврата.
    </Tile>
  )
}

/* ---------------- Offline (statement) ---------------- */

function Offline() {
  return (
    <section className="fs-tile fs-tile--offline">
      <span className="fs-offline-mark">
        <ShieldCheck size={19} strokeWidth={1.6} />
      </span>
      <div className="fs-offline-copy">
        <p className="fs-statement">Офлайн по умолчанию.</p>
        <p className="fs-statement-sub">Ни аккаунта, ни телеметрии, ни одного сетевого запроса.</p>
      </div>
      <span className="fs-orbit" aria-hidden />
    </section>
  )
}

export function BentoTiles() {
  return (
    <div className="fs-bento">
      <Duplicates />
      <ScanStat />
      <Snapshots />
      <CliTile />
      <Offline />
    </div>
  )
}
