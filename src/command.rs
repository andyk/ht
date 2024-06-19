use serde::{de::DeserializeOwned, Deserialize};

#[derive(Debug)]
pub enum Command {
    Input(String),
    GetView,
    Resize(usize, usize),
}

#[derive(Debug, Deserialize)]
struct InputArgs {
    payload: String,
}

#[derive(Debug, Deserialize)]
struct SendKeysArgs {
    keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ResizeArgs {
    cols: usize,
    rows: usize,
}

pub fn parse(line: &str) -> Result<Command, String> {
    serde_json::from_str::<serde_json::Value>(line)
        .map_err(|e| e.to_string())
        .and_then(build_command)
}

fn build_command(value: serde_json::Value) -> Result<Command, String> {
    match value["type"].as_str() {
        Some("input") => {
            let args: InputArgs = args_from_json_value(value)?;
            Ok(Command::Input(args.payload))
        }

        Some("sendKeys") => {
            let args: SendKeysArgs = args_from_json_value(value)?;
            let input = parse_keys(args.keys);
            Ok(Command::Input(input))
        }

        Some("resize") => {
            let args: ResizeArgs = args_from_json_value(value)?;
            Ok(Command::Resize(args.cols, args.rows))
        }

        Some("getView") => Ok(Command::GetView),

        other => Err(format!("invalid command type: {other:?}")),
    }
}

fn parse_keys(keys: Vec<String>) -> String {
    let keys: Vec<String> = keys.into_iter().map(parse_key).collect();

    keys.join("")
}

fn parse_key(key: String) -> String {
    let key = match key.as_str() {
        "C-@" => "\x00",
        "C-a" => "\x01",
        "C-b" => "\x02",
        "C-c" => "\x03",
        "C-d" => "\x04",
        "C-e" => "\x05",
        "C-f" => "\x06",
        "C-g" => "\x07",
        "C-h" => "\x08",
        "C-i" => "\x09",
        "C-j" => "\x0a",
        "C-k" => "\x0b",
        "C-l" => "\x0c",
        "C-m" => "\x0d",
        "C-n" => "\x0e",
        "C-o" => "\x0f",
        "C-p" => "\x10",
        "C-q" => "\x11",
        "C-r" => "\x12",
        "C-s" => "\x13",
        "C-t" => "\x14",
        "C-u" => "\x15",
        "C-v" => "\x16",
        "C-w" => "\x17",
        "C-x" => "\x18",
        "C-y" => "\x19",
        "C-z" => "\x1a",
        "C-[" => "\x1b",
        "C-\\" => "\x1c",
        "C-]" => "\x1d",
        "C-^" => "\x1e",
        "C--" => "\x1f",
        "C-Space" => "\x00", // same as C-@
        "Tab" => "\x09",     // same as C-i
        "Enter" => "\x0d",   // same as C-m
        "Escape" => "\x1b",  // same as C-[
        "C-/" => "\x1e",     // same as C-^
        "C-_" => "\x1f",     // same as C--
        _ => &key,
    };

    key.to_owned()
}

fn args_from_json_value<T>(value: serde_json::Value) -> Result<T, String>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod test {
    use super::parse;
    use super::Command;

    #[test]
    fn parse_input() {
        let command = parse(r#"{ "type": "input", "payload": "hello" }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "hello"));
    }

    #[test]
    fn parse_input_missing_args() {
        parse(r#"{ "type": "input" }"#).expect_err("should fail");
    }

    #[test]
    fn parse_send_keys() {
        let command = parse(r#"{ "type": "sendKeys", "keys": ["hello"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "hello"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-@"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x00"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-a"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x01"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-z"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1a"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-["] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-Space"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x00"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["Tab"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x09"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["Enter"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x0d"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["Escape"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b"));

        let command =
            parse(r#"{ "type": "sendKeys", "keys": ["hello", "Enter", "C-c"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "hello\x0d\x03"));
    }

    #[test]
    fn parse_send_keys_missing_args() {
        parse(r#"{ "type": "sendKeys" }"#).expect_err("should fail");
    }

    #[test]
    fn parse_resize() {
        let command = parse(r#"{ "type": "resize", "cols": 80, "rows": 24 }"#).unwrap();
        assert!(matches!(command, Command::Resize(80, 24)));
    }

    #[test]
    fn parse_resize_missing_args() {
        parse(r#"{ "type": "resize" }"#).expect_err("should fail");
    }

    #[test]
    fn parse_get_view() {
        let command = parse(r#"{ "type": "getView" }"#).unwrap();
        assert!(matches!(command, Command::GetView));
    }

    #[test]
    fn parse_invalid_json() {
        parse("{").expect_err("should fail");
    }
}
