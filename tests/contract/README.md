# Contract fixtures

Golden fixtures keep the Tauri command surface identical to the documented IPC
contract (`specs/002-native-ui-redesign/contracts/ipc.md`). Each file is named
`<command>.<scenario>.json` and contains:

```json
{
  "command": "tree_page",
  "input": {},
  "result": {},
  "error": null
}
```

Exactly one of `result` and `error` is non-null. Errors use the `E_*` envelope
from the IPC contract. Fixtures are written by T049 and verified by the
`contract_fixtures` test target; they must never contain real user data.

The `contract_fixtures` target validates the shape of every fixture in
`fixtures/` and is intentionally empty until the first command lands.
