use std::fs;
use std::path::Path;

use serde_json::Value;

/// Validates the envelope of every golden command fixture.
#[test]
fn fixtures_use_the_documented_envelope() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("contract/fixtures");
    let entries = fs::read_dir(&fixtures).expect("the contract fixtures directory exists");

    for entry in entries {
        let path = entry.expect("readable directory entry").path();

        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }

        let text = fs::read_to_string(&path).expect("fixture is readable");
        let value: Value = serde_json::from_str(&text).expect("fixture is valid JSON");

        assert!(
            value.get("command").and_then(Value::as_str).is_some(),
            "{} must declare a command name",
            path.display()
        );
        assert!(
            value.get("input").is_some(),
            "{} must declare the command input",
            path.display()
        );

        let has_result = value.get("result").is_some_and(|result| !result.is_null());
        let has_error = value.get("error").is_some_and(|error| !error.is_null());
        assert!(
            has_result ^ has_error,
            "{} must contain exactly one of `result` and `error`",
            path.display()
        );
    }
}
