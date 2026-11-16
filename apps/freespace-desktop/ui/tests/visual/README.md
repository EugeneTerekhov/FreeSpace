# Visual regression baselines

Screenshots are stored next to their specs as `*.png` and compared with a
`maxDiffPixelRatio` of 0.01. Baselines are captured on Windows with the bundled
Playwright Chromium at 1440×900 and device scale factor 1, in both themes and at
DPI 100/125/150/200% once the harness lands (T046).

Regenerate intentionally with:

```powershell
npm run test:visual -- --update-snapshots
```
