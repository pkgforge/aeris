//! Turning what a manager printed into records.

use std::collections::HashMap;

use serde_json::{Map, Value};

use super::manifest::{Format, Op};

/// Drop the escape sequences a manager writing for a terminal leaves behind.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();

    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }

        match chars.next() {
            // A control sequence runs until a byte in the @ to ~ range.
            Some('[') => {
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            // An operating system command carries a payload of its own, such
            // as the target of a hyperlink, and ends at a bell or at the
            // string terminator. Everything in between is addressed to the
            // terminal, not to the reader.
            Some(']') => {
                while let Some(next) = chars.next() {
                    match next {
                        '\u{7}' => break,
                        '\u{1b}' => {
                            // The string terminator is ESC followed by \.
                            if chars.next() == Some('\\') {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Any other escape takes only the byte after it.
            _ => {}
        }
    }

    out
}

/// Pick the records out of a JSON document.
///
/// Only what a manifest needs is understood: field access and a trailing
/// `[*]`, so `$.items[*]` and `$[*]` both work and nothing else is promised.
pub fn select<'a>(root: &'a Value, path: Option<&str>) -> Vec<&'a Value> {
    let Some(path) = path else {
        return match root {
            Value::Array(items) => items.iter().collect(),
            other => vec![other],
        };
    };

    let trimmed = path.trim().trim_start_matches('$');
    let wants_each = trimmed.ends_with("[*]");
    let trimmed = trimmed.trim_end_matches("[*]");

    let mut current = root;
    for segment in trimmed.split('.').filter(|s| !s.is_empty()) {
        match current.get(segment) {
            Some(next) => current = next,
            None => return Vec::new(),
        }
    }

    match current {
        Value::Array(items) if wants_each => items.iter().collect(),
        other => vec![other],
    }
}

/// Read everything a query operation printed.
pub fn records(op: &Op, stdout: &str, strip: bool) -> Result<Vec<Value>, String> {
    let cleaned;
    let stdout = if strip {
        cleaned = strip_ansi(stdout);
        cleaned.as_str()
    } else {
        stdout
    };

    match op.output.format {
        Format::Json => {
            let root: Value = serde_json::from_str(stdout.trim())
                .map_err(|e| format!("output was not json: {e}"))?;
            Ok(select(&root, op.output.select.as_deref())
                .into_iter()
                .cloned()
                .collect())
        }
        Format::Ndjson => Ok(stdout
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()),
        Format::KeyValue => Ok(key_value(op, stdout).into_iter().collect()),
        Format::Lines => {
            let pattern = op
                .pattern
                .as_deref()
                .ok_or_else(|| "the operation reads lines but has no pattern".to_string())?;
            let pattern =
                regex::Regex::new(pattern).map_err(|e| format!("unreadable pattern: {e}"))?;

            Ok(stdout
                .lines()
                .skip(op.output.skip_header)
                .filter_map(|line| captures(&pattern, line))
                .collect())
        }
    }
}

/// Read a block of named values as the one thing it describes.
///
/// A name given more than once, as a manager listing several dependencies
/// would, is kept as a list rather than the last one winning.
fn key_value(op: &Op, printed: &str) -> Option<Value> {
    let separator = op.output.separator.as_deref().unwrap_or(":");
    let mut record = Map::new();

    for line in printed.lines().skip(op.output.skip_header) {
        let Some((name, value)) = line.split_once(separator) else {
            continue;
        };

        let name = name.trim();
        let value = value.trim();
        if name.is_empty() || value.is_empty() {
            continue;
        }

        match record.remove(name) {
            Some(Value::Array(mut seen)) => {
                seen.push(Value::String(value.to_string()));
                record.insert(name.to_string(), Value::Array(seen));
            }
            Some(first) => {
                record.insert(
                    name.to_string(),
                    Value::Array(vec![first, Value::String(value.to_string())]),
                );
            }
            None => {
                record.insert(name.to_string(), Value::String(value.to_string()));
            }
        }
    }

    (!record.is_empty()).then_some(Value::Object(record))
}

fn captures(pattern: &regex::Regex, line: &str) -> Option<Value> {
    let found = pattern.captures(line)?;
    let mut record = Map::new();

    for name in pattern.capture_names().flatten() {
        if let Some(matched) = found.name(name) {
            record.insert(
                name.to_string(),
                Value::String(matched.as_str().to_string()),
            );
        }
    }

    (!record.is_empty()).then_some(Value::Object(record))
}

/// Read a field by the name the manager itself reports it under.
///
/// Unlike the mapped fields, this is for values aeris has no name for and
/// only passes along.
pub fn value(record: &Value, name: &str) -> Option<String> {
    let mapped = HashMap::from([(name.to_string(), name.to_string())]);
    text(record, &mapped, name)
}

/// Read one field of a record, under the name the manifest maps it to.
///
/// A manager reporting several of something, such as more than one homepage,
/// reads back as a list rather than as the JSON it was written in.
pub fn text(record: &Value, fields: &HashMap<String, String>, key: &str) -> Option<String> {
    let name = fields.get(key)?;
    match record.get(name)? {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        Value::Array(items) => {
            let joined = items
                .iter()
                .filter_map(|item| match item {
                    Value::String(s) => Some(s.clone()),
                    Value::Null => None,
                    other => Some(other.to_string()),
                })
                .collect::<Vec<_>>()
                .join(", ");

            (!joined.is_empty()).then_some(joined)
        }
        other => Some(other.to_string()),
    }
}

pub fn number(record: &Value, fields: &HashMap<String, String>, key: &str) -> Option<u64> {
    let name = fields.get(key)?;
    let value = record.get(name)?;
    value.as_u64().or_else(|| value.as_str().and_then(size))
}

/// Read a size a manager wrote out for a person, such as `247.54 KiB`.
///
/// Only the JSON formats carry a real number; a line or a key and its value
/// are text, and a manager writing those has usually already turned the size
/// into something readable. A bare number is taken to be bytes.
fn size(text: &str) -> Option<u64> {
    let text = text.trim();
    let end = text
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != ',')
        .unwrap_or(text.len());
    let (amount, unit) = text.split_at(end);
    let amount: f64 = amount.replace(',', "").parse().ok()?;

    let scale = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" | "byte" | "bytes" => 1.0,
        "k" | "kib" => 1024.0,
        "kb" => 1e3,
        "m" | "mib" => 1024.0 * 1024.0,
        "mb" => 1e6,
        "g" | "gib" => 1024.0 * 1024.0 * 1024.0,
        "gb" => 1e9,
        "t" | "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "tb" => 1e12,
        _ => return None,
    };

    let bytes = amount * scale;
    (bytes.is_finite() && bytes >= 0.0).then(|| bytes.round() as u64)
}

pub fn flag(record: &Value, fields: &HashMap<String, String>, key: &str) -> Option<bool> {
    let name = fields.get(key)?;
    record.get(name)?.as_bool()
}

/// Fill `{key}` placeholders, refusing a template it cannot complete.
pub fn fill(template: &str, values: &HashMap<String, String>) -> Option<String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(open) = rest.find('{') {
        let close = rest[open..].find('}')? + open;
        out.push_str(&rest[..open]);

        let value = values.get(&rest[open + 1..close])?;
        if value.is_empty() {
            return None;
        }
        out.push_str(value);

        rest = &rest[close + 1..];
    }

    out.push_str(rest);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::command::manifest;

    fn op(body: &str) -> Op {
        let text = format!(
            r#"
schema_version = 1
id = "demo"
name = "Demo"

[detect]
command = "demo"

[ops.demo]
{body}
"#
        );
        manifest::parse(&text)
            .expect("should read")
            .op("demo")
            .cloned()
            .expect("should have the operation")
    }

    #[test]
    fn a_json_document_is_read_through_its_path() {
        let op = op(r#"args = ["x"]
output = { format = "json", select = "$.items[*]" }"#);
        let found = records(
            &op,
            r#"{"items":[{"name":"a"},{"name":"b"}],"total":2}"#,
            false,
        )
        .expect("should read");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0]["name"], "a");
    }

    #[test]
    fn a_path_that_matches_nothing_reads_as_empty() {
        let op = op(r#"args = ["x"]
output = { format = "json", select = "$.missing[*]" }"#);
        let found = records(&op, r#"{"items":[]}"#, false).expect("should read");
        assert!(found.is_empty());
    }

    #[test]
    fn a_stream_reads_a_record_a_line() {
        let op = op(r#"args = ["x"]
output = { format = "ndjson" }"#);
        let found =
            records(&op, "{\"type\":\"a\"}\n\n{\"type\":\"b\"}\n", false).expect("should read");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn plain_text_is_read_through_its_pattern() {
        let op = op(r#"args = ["x"]
output = { format = "lines", skip_header = 1 }
pattern = "^(?P<name>\\S+)\\s+(?P<version>\\S+)$""#);
        let found = records(&op, "NAME VERSION\nfoo 1.0\nbar 2.0\n", false).expect("should read");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0]["name"], "foo");
        assert_eq!(found[1]["version"], "2.0");
    }

    #[test]
    fn colour_is_removed_before_a_pattern_runs() {
        let op = op(r#"args = ["x"]
output = { format = "lines" }
pattern = "^(?P<name>\\S+)$""#);
        let found = records(&op, "\u{1b}[32mfoo\u{1b}[0m\n", true).expect("should read");
        assert_eq!(found[0]["name"], "foo");
    }

    #[test]
    fn several_of_something_reads_as_a_list() {
        let record: Value = serde_json::from_str(
            r#"{"homepages":["https://a.example","https://b.example"],"licenses":[],"one":["MIT"]}"#,
        )
        .unwrap();
        let fields = HashMap::from([
            ("homepage".to_string(), "homepages".to_string()),
            ("license".to_string(), "licenses".to_string()),
            ("single".to_string(), "one".to_string()),
        ]);

        assert_eq!(
            text(&record, &fields, "homepage").as_deref(),
            Some("https://a.example, https://b.example")
        );
        assert_eq!(text(&record, &fields, "single").as_deref(), Some("MIT"));
        // Nothing to show is not the same as an empty list to show.
        assert_eq!(text(&record, &fields, "license"), None);
    }

    #[test]
    fn a_hyperlink_leaves_only_the_words_it_wrapped() {
        // Pacstall marks up its repository names as clickable links, and the
        // address is addressed to the terminal rather than to a reader.
        let linked = concat!(
            "\u{1b}[0;32mneofetch \u{1b}[0;35m@ \u{1b}[0;36m",
            "\u{1b}]8;;https://github.com/pacstall/pacstall-programs/tree/master\u{7}",
            "github:pacstall/pacstall-programs",
            "\u{1b}]8;;\u{7} \u{1b}[0m"
        );

        assert_eq!(
            strip_ansi(linked).trim(),
            "neofetch @ github:pacstall/pacstall-programs"
        );
    }

    #[test]
    fn a_hyperlink_closed_the_other_way_is_understood_too() {
        // The string terminator is as valid an ending as a bell.
        let linked = "\u{1b}]8;;https://example.invalid\u{1b}\\name\u{1b}]8;;\u{1b}\\";
        assert_eq!(strip_ansi(linked), "name");
    }

    #[test]
    fn a_block_of_named_values_reads_as_one_thing() {
        let op = op(r#"args = ["x"]
output = { format = "keyvalue", separator = "=" }"#);

        // As pacstall prints it, tabs and repeated names and all.
        let printed = concat!(
            "--- github:pacstall/pacstall-programs ---\n",
            "pkgbase = hello-rhino-bin\n",
            "\tgives = hello-rhino\n",
            "\tpkgver = 2025.2\n",
            "\tpkgdesc = Rhino Linux Welcome Screen\n",
            "\tdepends = libssl-dev\n",
            "\tdepends = gettext\n"
        );

        let found = records(&op, printed, false).expect("should read");
        assert_eq!(found.len(), 1, "a block describes one thing");
        assert_eq!(found[0]["pkgver"], "2025.2");
        assert_eq!(found[0]["pkgdesc"], "Rhino Linux Welcome Screen");
        // The line with no separator is not a named value.
        assert!(found[0].get("--- github").is_none());
        // Named twice, so kept as both.
        assert_eq!(
            found[0]["depends"],
            serde_json::json!(["libssl-dev", "gettext"])
        );
    }

    #[test]
    fn a_colon_is_what_separates_a_name_by_default() {
        let op = op(r#"args = ["x"]
output = { format = "keyvalue" }"#);

        let found =
            records(&op, "name: hello\nversion: 1.2\nempty:\n", false).expect("should read");
        assert_eq!(found[0]["version"], "1.2");
        // Nothing after the separator is nothing to record.
        assert!(found[0].get("empty").is_none());
    }

    #[test]
    fn nothing_named_reads_as_nothing() {
        let op = op(r#"args = ["x"]
output = { format = "keyvalue" }"#);

        assert!(records(&op, "no names here\n", false).unwrap().is_empty());
    }

    #[test]
    fn a_template_takes_the_first_form_it_can_complete() {
        let values = HashMap::from([
            ("name".to_string(), "cat".to_string()),
            ("family".to_string(), String::new()),
        ]);
        assert_eq!(fill("{family}/{name}", &values), None);
        assert_eq!(fill("{name}", &values).as_deref(), Some("cat"));
        assert_eq!(fill("{repo}", &values), None);
        assert_eq!(
            fill("no placeholder", &values).as_deref(),
            Some("no placeholder")
        );
    }

    #[test]
    fn a_size_written_for_a_person_is_read_as_bytes() {
        assert_eq!(size("247.54 KiB"), Some(253_481));
        assert_eq!(size("2.17 MiB"), Some(2_275_410));
        assert_eq!(size("1GiB"), Some(1_073_741_824));
        assert_eq!(size("1 kB"), Some(1_000));
        assert_eq!(size("512"), Some(512));
        assert_eq!(size("1,024 bytes"), Some(1_024));
        assert_eq!(size("unknown"), None);
        assert_eq!(size("-"), None);
        assert_eq!(size("12 parsecs"), None);
    }

    #[test]
    fn a_number_field_reads_both_a_number_and_a_written_size() {
        let fields = HashMap::from([("size".to_string(), "size".to_string())]);
        assert_eq!(
            number(&serde_json::json!({"size": 4096}), &fields, "size"),
            Some(4096)
        );
        assert_eq!(
            number(&serde_json::json!({"size": "247.54 KiB"}), &fields, "size"),
            Some(253_481)
        );
        assert_eq!(
            number(&serde_json::json!({"size": "unknown"}), &fields, "size"),
            None
        );
    }
}
