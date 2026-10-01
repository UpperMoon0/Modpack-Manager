use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

fn whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | 12)
}

fn property_key(line: &[u8]) -> Result<String> {
    let mut key = String::new();
    let mut index = 0;
    while index < line.len() {
        let byte = line[index];
        if whitespace(byte) || matches!(byte, b'=' | b':') {
            break;
        }
        index += 1;
        if byte != b'\\' {
            key.push(char::from(byte));
            continue;
        }
        let Some(&escaped) = line.get(index) else {
            break;
        };
        index += 1;
        if escaped == b'u' {
            let digits = line
                .get(index..index + 4)
                .context("invalid properties Unicode escape")?;
            let code = u16::from_str_radix(std::str::from_utf8(digits)?, 16)
                .context("invalid properties Unicode escape")?;
            key.push(char::from_u32(u32::from(code)).context("invalid properties key character")?);
            index += 4;
        } else {
            key.push(match escaped {
                b't' => '\t',
                b'n' => '\n',
                b'r' => '\r',
                b'f' => '\u{c}',
                other => char::from(other),
            });
        }
    }
    Ok(key)
}

fn scalar(value: &Value) -> Result<String> {
    match value {
        Value::Bool(_) | Value::Number(_) => Ok(value.to_string()),
        Value::String(value) => {
            let mut escaped = String::new();
            for unit in value.encode_utf16() {
                match unit {
                    32 => escaped.push_str("\\ "),
                    92 => escaped.push_str("\\\\"),
                    9 => escaped.push_str("\\t"),
                    10 => escaped.push_str("\\n"),
                    13 => escaped.push_str("\\r"),
                    33..=126 => escaped.push(char::from_u32(u32::from(unit)).unwrap()),
                    other => escaped.push_str(&format!("\\u{other:04x}")),
                }
            }
            Ok(escaped)
        }
        _ => bail!("patchProperties supports only scalar values"),
    }
}

fn natural_lines(source: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < source.len() {
        match source[index] {
            b'\r' => {
                index += 1;
                if source.get(index) == Some(&b'\n') {
                    index += 1;
                }
            }
            b'\n' => index += 1,
            _ => {
                index += 1;
                continue;
            }
        }
        lines.push(&source[start..index]);
        start = index;
    }
    if start < source.len() {
        lines.push(&source[start..]);
    }
    lines
}

fn line_ending(line: &[u8]) -> &[u8] {
    if line.ends_with(b"\r\n") {
        b"\r\n"
    } else if line.ends_with(b"\r") {
        b"\r"
    } else if line.ends_with(b"\n") {
        b"\n"
    } else {
        b""
    }
}

/// Preserve unowned properties byte for byte, including comments and continuation lines.
pub(crate) fn render_patched_properties(
    source: &[u8],
    values: &BTreeMap<String, Value>,
) -> Result<Vec<u8>> {
    let newline: &[u8] = if source.windows(2).any(|pair| pair == b"\r\n") {
        b"\r\n"
    } else if source.contains(&b'\r') {
        b"\r"
    } else {
        b"\n"
    };
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    let lines = natural_lines(source);
    let mut index = 0;
    while index < lines.len() {
        let start = index;
        let mut logical = Vec::new();
        let mut ending: &[u8];
        loop {
            let physical = lines[index];
            ending = line_ending(physical);
            let mut body = &physical[..physical.len() - ending.len()];
            body = body.trim_ascii_start();
            let comment =
                logical.is_empty() && body.first().is_some_and(|byte| matches!(byte, b'#' | b'!'));
            let continued =
                !comment && body.iter().rev().take_while(|byte| **byte == b'\\').count() % 2 == 1;
            logical.extend_from_slice(if continued {
                &body[..body.len() - 1]
            } else {
                body
            });
            index += 1;
            if !continued || index == lines.len() {
                break;
            }
        }
        let comment = logical
            .first()
            .is_none_or(|byte| matches!(byte, b'#' | b'!'));
        let key = if comment {
            String::new()
        } else {
            property_key(&logical)?
        };
        if let Some(value) = values.get(&key) {
            seen.insert(key.clone());
            output.extend_from_slice(format!("{key}={}", scalar(value)?).as_bytes());
            output.extend_from_slice(ending);
        } else {
            for physical in &lines[start..index] {
                output.extend_from_slice(physical);
            }
        }
    }
    for (key, value) in values {
        if seen.contains(key) {
            continue;
        }
        if let Some(last) = natural_lines(&output).last() {
            let ending = line_ending(last);
            let body = &last[..last.len() - ending.len()];
            let continued = body.iter().rev().take_while(|byte| **byte == b'\\').count() % 2 == 1;
            if ending.is_empty() {
                output.extend_from_slice(newline);
            }
            // An empty natural line closes any unfinished continuation without
            // changing the preceding property's value as read at EOF by Java.
            if continued {
                output.extend_from_slice(newline);
            }
        }
        output.extend_from_slice(
            format!("{key}={}{}", scalar(value)?, std::str::from_utf8(newline)?).as_bytes(),
        );
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_duplicate_escaped_and_continued_keys_preserving_unowned_bytes() {
        let source = b"# settings\r\nmotd=keep\\\r\n  online-mode=true\r\nlevel-name=my-world\r\nonline\\u002dmode:true\r\nonline-mode=tr\\\r\n ue\r\nopaque=\xff\r\n";
        let values = BTreeMap::from([("online-mode".into(), Value::Bool(false))]);
        let patched = render_patched_properties(source, &values).unwrap();
        assert_eq!(patched, b"# settings\r\nmotd=keep\\\r\n  online-mode=true\r\nlevel-name=my-world\r\nonline-mode=false\r\nonline-mode=false\r\nopaque=\xff\r\n");
        assert_eq!(
            render_patched_properties(&patched, &values).unwrap(),
            patched
        );
    }

    #[test]
    fn appends_missing_key_without_joining_the_previous_line() {
        let values = BTreeMap::from([("online-mode".into(), Value::Bool(false))]);
        assert_eq!(
            render_patched_properties(b"server-port=25570", &values).unwrap(),
            b"server-port=25570\nonline-mode=false\n"
        );
        assert_eq!(
            render_patched_properties(b"", &values).unwrap(),
            b"online-mode=false\n"
        );
    }
}
