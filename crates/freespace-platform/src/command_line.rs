//! Windows command-line quoting shared by Explorer integration and elevation.
//!
//! The rules follow `CommandLineToArgvW`: an argument is quoted when it is empty
//! or contains whitespace or a double quote, and backslashes are doubled in
//! front of the closing quote. Keeping this code outside the optional
//! context-menu module lets portable builds reuse it for UAC relaunches.

/// Quotes one argument for a Windows command line.
#[must_use]
pub fn quote_argument(argument: &str) -> String {
    if !argument.is_empty()
        && !argument
            .chars()
            .any(|character| character == ' ' || character == '\t' || character == '"')
    {
        return argument.to_owned();
    }

    let mut quoted = String::with_capacity(argument.len() + 2);
    quoted.push('"');
    let mut backslashes = 0usize;
    for character in argument.chars() {
        match character {
            '\\' => {
                backslashes += 1;
                quoted.push('\\');
            }
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                quoted.push('\\');
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                backslashes = 0;
                quoted.push(character);
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes));
    quoted.push('"');

    quoted
}

/// Quotes a filesystem path without lossy Unicode conversion.
#[must_use]
pub fn quote_os(path: &std::ffi::OsStr) -> String {
    quote_argument(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::quote_argument;

    #[test]
    fn quotes_only_when_required() {
        assert_eq!(
            quote_argument("C:\\Tools\\freespace.exe"),
            "C:\\Tools\\freespace.exe"
        );
        assert_eq!(
            quote_argument("C:\\Program Files\\FreeSpace\\freespace.exe"),
            "\"C:\\Program Files\\FreeSpace\\freespace.exe\""
        );
        assert_eq!(quote_argument(""), "\"\"");
        assert_eq!(quote_argument("%1"), "%1");
    }

    #[test]
    fn escapes_quotes_and_trailing_backslashes() {
        assert_eq!(quote_argument("say \"hi\""), r#""say \"hi\"""#);
        assert_eq!(
            quote_argument(r"C:\Program Files\FreeSpace\"),
            "\"C:\\Program Files\\FreeSpace\\\\\""
        );
        assert_eq!(quote_argument("a\"b"), r#""a\"b""#);
    }

    #[test]
    fn keeps_unicode_paths_intact() {
        let path = r"C:\Пользователи\测试 目录\freespace.exe";
        let quoted = quote_argument(path);

        assert!(quoted.starts_with('"'));
        assert!(quoted.ends_with('"'));
        assert!(quoted.contains("测试 目录"));
        assert_eq!(
            quote_argument(OsStr::new("a b").to_string_lossy().as_ref()),
            "\"a b\""
        );
    }
}
