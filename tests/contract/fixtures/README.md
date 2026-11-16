# Golden command fixtures

Add one `<command>.<scenario>.json` file per documented success and error path.
The `contract_fixtures` test target rejects fixtures without a `command`, an
`input`, or with both/neither `result` and `error`.
