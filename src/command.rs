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
    match key.as_str() {
        "C-@" => "\x00".to_owned(),
        "C-[" => "\x1b".to_owned(),
        "C-\\" => "\x1c".to_owned(),
        "C-]" => "\x1d".to_owned(),
        "C-^" => "\x1e".to_owned(),
        "C--" => "\x1f".to_owned(),
        "C-Space" => "\x00".to_owned(), // same as C-@
        "Tab" => "\x09".to_owned(),     // same as C-i
        "Enter" => "\x0d".to_owned(),   // same as C-m
        "Escape" => "\x1b".to_owned(),  // same as C-[
        "C-/" => "\x1e".to_owned(),     // same as C-^
        "C-_" => "\x1f".to_owned(),     // same as C--

        k => {
            let chars: Vec<char> = k.chars().collect();

            match chars.as_slice() {
                ['C', '-', k @ 'a'..='z'] => ((*k as u8 - 0x60) as char).to_string(),
                ['A', '-', k] => format!("\x1b{}", k),
                _ => key,
            }
        }
    }
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

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-a"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1ba"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-A"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1bA"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-z"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1bz"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-Z"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1bZ"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-1"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b1"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-!"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b!"));

        let command =
            parse(r#"{ "type": "sendKeys", "keys": ["hello", "Enter", "C-c", "A-^"] }"#).unwrap();

        assert!(matches!(command, Command::Input(input) if input == "hello\x0d\x03\x1b^"));
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
