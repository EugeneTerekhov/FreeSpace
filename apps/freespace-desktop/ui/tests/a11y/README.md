# Accessibility checks

Accessibility specs use `@axe-core/playwright` together with keyboard-only
scenarios. The shared helper (axe configuration, focus order, live-region
assertions) is added in T047; specs in this directory are executed with:

```powershell
npm run test:a11y
```
