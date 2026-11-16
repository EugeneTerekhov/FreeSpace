import { useState } from 'react'
import {
  Activity,
  Archive,
  ArrowRight,
  ArrowUpRight,
  Check,
  Eraser,
  FileSearch,
  FolderTree,
  KeyRound,
  PenLine,
  RotateCcw,
  ShieldCheck,
  Terminal,
  Users,
} from 'lucide-react'
import { Caret, Stage, Window } from './kit'
import { HeroScene } from './scenes/HeroScene'
import { BentoTiles } from './scenes/BentoTiles'

const METRICS = [
  { value: '10 млн', label: 'узлов в дереве' },
  { value: '≤ 200 мс', label: 'отклик интерфейса' },
  { value: '22', label: 'команды CLI с JSON' },
  { value: '24', label: 'экрана в одном окне' },
]

const FEATURES = [
  { icon: Activity, title: 'Мониторинг записи', text: 'ETW и резервный режим по счётчикам.' },
  { icon: FolderTree, title: 'Длинные пути', text: 'Verbatim-префиксы вместо лимита 260.' },
  { icon: Archive, title: 'Архивы', text: 'ZIP, 7z, TAR, GZ и ISO без распаковки.' },
  { icon: Users, title: 'Владельцы', text: 'Отчёт по NTFS-владельцам и объёмам.' },
  { icon: KeyRound, title: 'Права и атрибуты', text: 'Пакетно, с предпросмотром и откатом.' },
  { icon: Eraser, title: 'Шредер', text: 'С предупреждением об SSD и TRIM.' },
  { icon: FileSearch, title: 'Метаданные', text: 'EXIF, аудио, видео и PE.' },
  { icon: PenLine, title: 'Переименование', text: 'Массово, с предпросмотром.' },
]

const COMMANDS = [
  { name: 'tree', text: 'дерево с поиском и сортировкой' },
  { name: 'duplicates', text: 'группы с побайтовой проверкой' },
  { name: 'cleanup suggest', text: 'рекомендации с оценкой риска' },
  { name: 'snapshot diff', text: 'что выросло между снимками' },
  { name: 'schedule', text: 'задание Планировщика из профиля' },
]

function SiteHeader() {
  return (
    <header className="site-header">
      <a className="site-brand" href="#top" aria-label="FreeSpace — на главную">
        <img className="site-mark" src="/icon.png" alt="" />
        <span className="site-wordmark">FreeSpace</span>
      </a>
      <nav className="site-nav" aria-label="Разделы страницы">
        <a href="#features">Возможности</a>
        <a href="#cli">CLI</a>
        <a href="#build">Сборка</a>
      </nav>
      <a className="site-header-cta" href="#build">
        Собрать
        <ArrowRight size={14} strokeWidth={1.7} />
      </a>
    </header>
  )
}

function SiteHero() {
  const [run, setRun] = useState(0)
  return (
    <section className="hero" id="top">
      <div className="wrap">
        <div className="hero-copy">
          <p className="eyebrow">
            <span className="eyebrow-dot" />
            Native Windows · Rust · Offline
          </p>
          <h1>
            Каждый гигабайт — <span className="grad">на виду.</span>
          </h1>
          <p className="hero-sub">
            FreeSpace строит карту диска по MFT, подтверждает дубликаты побайтно, сравнивает снимки и
            показывает, что можно вернуть. Одна портативная сборка для Windows.
          </p>
          <div className="hero-actions">
            <a className="btn btn-primary" href="#features">
              Посмотреть возможности
              <ArrowRight size={16} strokeWidth={1.8} />
            </a>
            <a className="btn btn-secondary" href="#cli">
              <Terminal size={16} strokeWidth={1.7} />
              Команды CLI
            </a>
          </div>
          <p className="hero-note">Windows 10 21H2+ / Windows 11 x64 · портативная сборка</p>
        </div>

        <div className="hero-figure">
          <div className="hero-stage hero-stage--wide">
            <HeroScene key={`wide-${run}`} variant="wide" />
          </div>
          <div className="hero-stage hero-stage--medium">
            <HeroScene key={`medium-${run}`} variant="medium" />
          </div>
          <div className="hero-stage hero-stage--compact">
            <HeroScene key={`compact-${run}`} variant="compact" />
          </div>
        </div>
        <div className="hero-figure-foot">
          <button type="button" className="replay" onClick={() => setRun((n) => n + 1)}>
            <RotateCcw size={13} strokeWidth={1.8} />
            Повторить сцену
          </button>
        </div>
      </div>
    </section>
  )
}

function Metrics() {
  return (
    <div className="wrap">
      <div className="metrics">
        {METRICS.map((metric) => (
          <div key={metric.label} className="metric">
            <span className="metric-value">{metric.value}</span>
            <span className="metric-label">{metric.label}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

function Features() {
  return (
    <section className="section features" id="features">
      <div className="wrap">
        <div className="section-head">
          <p className="eyebrow">
            <span className="eyebrow-dot" />
            Возможности
          </p>
          <h2>Не один экран, а весь диск.</h2>
          <p>
            Проверка, сравнение, очистка и автоматизация — в одном окне. Всё, что видно на карте, можно
            повторить в терминале.
          </p>
        </div>

        <BentoTiles />

        <div className="feature-list">
          {FEATURES.map((feature) => {
            const Glyph = feature.icon
            return (
              <div key={feature.title} className="feature-item">
                <span className="feature-icon">
                  <Glyph size={16} strokeWidth={1.6} />
                </span>
                <span className="feature-text">
                  <b>{feature.title}</b>
                  <span>{feature.text}</span>
                </span>
              </div>
            )
          })}
        </div>
      </div>
    </section>
  )
}

function CliTerminal() {
  return (
    <Stage
      width={540}
      height={380}
      compact={{ below: 480, width: 358, height: 340 }}
      theme="dark"
      className="fs-term-stage"
      tokens={{
        '--ss-term-bg': '#0b0912',
        '--ss-term-path': '#ff9a52',
        '--ss-term-sigil': '#ff9a52',
        '--ss-term-ok': '#7ee8a9',
        '--ss-term-err': '#f87171',
        '--ss-term-dim': '#676074',
        '--ss-shadow-hero': '0 0 0 1px rgba(255,255,255,0.07), 0 40px 80px -30px rgba(0,0,0,0.9)',
      }}
    >
      {(_s, _rect, layout) => (
        <Window
          chrome="terminal"
          title="freespace — PowerShell"
          lights="grey"
          className="fs-term-window"
          body={{ className: 'fs-term-body' }}
        >
          <div className="ss-term__line">
            <span className="ss-term__path">PS C:\</span>
            <span className="fs-term-sigil">&gt;</span>
            <span className="ss-term__cmd"> freespace scan C:\ --json</span>
          </div>
          <div className="fs-term-out">
            {'{"volume":"C:","files":1204611,"bytes":764706832384}'}
          </div>
          <div className="ss-term__line">
            <span className="ss-term__path">PS C:\</span>
            <span className="fs-term-sigil">&gt;</span>
            <span className="ss-term__cmd">
              {layout === 'compact' ? ' freespace cleanup suggest C:\ --json' : ' freespace cleanup suggest C:\ --older-than 90 --json'}
            </span>
          </div>
          <div className="fs-term-out">{'{"candidates":41,"reclaimable_bytes":334946304000}'}</div>
          <div className="ss-term__line">
            <span className="ss-term__path">PS C:\</span>
            <span className="fs-term-sigil">&gt;</span>
            <span className="ss-term__cmd"> freespace drives --json</span>
          </div>
          <div className="fs-term-out">
            {layout === 'compact'
              ? '{"volumes":[{"mount":"C:"},{"mount":"D:"}]}'
              : '{"volumes":[{"mount":"C:","free_bytes":309237645312},{"mount":"D:"}]}'}
          </div>
          <div className="ss-term__line fs-term-last">
            <span className="ss-term__path">PS C:\</span>
            <span className="fs-term-sigil">&gt;</span>
            <span className="fs-term-caret-gap">
              <Caret />
            </span>
          </div>
        </Window>
      )}
    </Stage>
  )
}

function CliSection() {
  return (
    <section className="section cli-section" id="cli">
      <div className="wrap cli-grid">
        <div className="cli-copy">
          <p className="eyebrow">
            <span className="eyebrow-dot" />
            CLI
          </p>
          <h2>Один движок. Два интерфейса.</h2>
          <p className="section-lead">
            Те же сканы, фильтры и безопасные операции — из PowerShell. JSON для скриптов, коды возврата
            для CI и блокировка параллельных запусков.
          </p>
          <ul className="command-list">
            {COMMANDS.map((command) => (
              <li key={command.name} className="command-row">
                <code className="command-name">{command.name}</code>
                <span className="command-text">{command.text}</span>
              </li>
            ))}
          </ul>
          <a className="text-link" href="/docs/cli.md" target="_blank" rel="noreferrer">
            Контракт CLI
            <ArrowUpRight size={14} strokeWidth={1.8} />
          </a>
        </div>
        <div className="cli-figure">
          <CliTerminal />
        </div>
      </div>
    </section>
  )
}

function Build() {
  return (
    <section className="section build" id="build">
      <div className="wrap build-grid">
        <div className="build-copy">
          <p className="eyebrow">
            <span className="eyebrow-dot" />
            Сборка
          </p>
          <h2>Заберите место обратно.</h2>
          <p className="section-lead">
            Rust 1.85+, edition 2024. Портативная сборка — одна папка рядом с исполняемым файлом, без
            установщика и записей в реестре.
          </p>
          <div className="build-facts">
            <span>
              <Check size={14} strokeWidth={2} /> Windows 10 21H2+ / 11 x64
            </span>
            <span>
              <Check size={14} strokeWidth={2} /> GUI и CLI из одного workspace
            </span>
            <span>
              <Check size={14} strokeWidth={2} /> Без сети и телеметрии
            </span>
          </div>
          <div className="build-links">
            <a className="btn btn-primary" href="/docs/README.md" target="_blank" rel="noreferrer">
              Открыть README
              <ArrowUpRight size={15} strokeWidth={1.8} />
            </a>
            <a className="btn btn-secondary" href="/docs/user-guide.md" target="_blank" rel="noreferrer">
              Руководство пользователя
            </a>
          </div>
        </div>

        <div className="build-code">
          <div className="build-code-head">
            <span>PowerShell</span>
          </div>
          <pre className="build-code-body">
            <span className="code-prompt">PS&gt;</span> cargo build --workspace{'\n'}
            <span className="code-prompt">PS&gt;</span> cargo run -p freespace-gui{'\n'}
            <span className="code-prompt">PS&gt;</span> cargo test --workspace
          </pre>
        </div>
      </div>
    </section>
  )
}

function SiteFooter() {
  return (
    <footer className="site-footer">
      <div className="wrap footer-grid">
        <div className="footer-brand">
          <a className="site-brand" href="#top">
            <img className="site-mark" src="/icon.png" alt="" />
            <span className="site-wordmark">FreeSpace</span>
          </a>
          <p>Нативная утилита для Windows: карта диска, дубликаты, снимки и безопасная очистка.</p>
        </div>
        <div className="footer-col">
          <span className="footer-title">Продукт</span>
          <a href="#features">Возможности</a>
          <a href="#cli">CLI</a>
          <a href="#build">Сборка</a>
        </div>
        <div className="footer-col">
          <span className="footer-title">Документы</span>
          <a href="/docs/README.md" target="_blank" rel="noreferrer">
            README
          </a>
          <a href="/docs/user-guide.md" target="_blank" rel="noreferrer">
            Руководство
          </a>
          <a href="/docs/cli.md" target="_blank" rel="noreferrer">
            CLI-контракт
          </a>
        </div>
        <div className="footer-col">
          <span className="footer-title">Стек</span>
          <span>Rust · egui / eframe</span>
          <span>NTFS · MFT · USN</span>
          <span>Windows 10 / 11</span>
        </div>
      </div>
      <div className="wrap footer-bottom">
        <span>© 2026 FreeSpace</span>
        <span className="footer-note">
          <ShieldCheck size={13} strokeWidth={1.7} />
          Офлайн по умолчанию
        </span>
      </div>
    </footer>
  )
}

export default function App() {
  return (
    <>
      <div className="page-glow" aria-hidden />
      <SiteHeader />
      <main>
        <SiteHero />
        <Metrics />
        <Features />
        <CliSection />
        <Build />
      </main>
      <SiteFooter />
    </>
  )
}
