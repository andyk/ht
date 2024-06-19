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
    let mut s = String::new();

    match key.as_str() {
        "C-@" | "C-Space" | "^@" => "\x00",
        "C-[" | "Escape" | "^[" => "\x1b",
        "C-\\" | "^\\" => "\x1c",
        "C-]" | "^]" => "\x1d",
        "C-^" | "C-/" => "\x1e",
        "C--" | "C-_" => "\x1f",
        "Tab" => "\x09",   // same as C-i
        "Enter" => "\x0d", // same as C-m
        "Left" => "\x1b[D",
        "Right" => "\x1b[C",
        "Up" => "\x1b[A",
        "Down" => "\x1b[B",
        "C-Left" => "\x1b[1;5D",
        "C-Right" => "\x1b[1;5C",
        "S-Left" => "\x1b[1;2D",
        "S-Right" => "\x1b[1;2C",
        "C-Up" => "\x1b[1;5A",
        "C-Down" => "\x1b[1;5B",
        "S-Up" => "\x1b[1;2A",
        "S-Down" => "\x1b[1;2B",
        "A-Left" => "\x1b[1;3D",
        "A-Right" => "\x1b[1;3C",
        "A-Up" => "\x1b[1;3A",
        "A-Down" => "\x1b[1;3B",
        "C-S-Left" | "S-C-Left" => "\x1b[1;6D",
        "C-S-Right" | "S-C-Right" => "\x1b[1;6C",
        "C-S-Up" | "S-C-Up" => "\x1b[1;6A",
        "C-S-Down" | "S-C-Down" => "\x1b[1;6B",
        "C-A-Left" | "A-C-Left" => "\x1b[1;7D",
        "C-A-Right" | "A-C-Right" => "\x1b[1;7C",
        "C-A-Up" | "A-C-Up" => "\x1b[1;7A",
        "C-A-Down" | "A-C-Down" => "\x1b[1;7B",
        "A-S-Left" | "S-A-Left" => "\x1b[1;4D",
        "A-S-Right" | "S-A-Right" => "\x1b[1;4C",
        "A-S-Up" | "S-A-Up" => "\x1b[1;4A",
        "A-S-Down" | "S-A-Down" => "\x1b[1;4B",
        "C-A-S-Left" | "C-S-A-Left" | "A-C-S-Left" | "S-C-A-Left" | "A-S-C-Left" | "S-A-C-Left" => {
            "\x1b[1;8D"
        }
        "C-A-S-Right" | "C-S-A-Right" | "A-C-S-Right" | "S-C-A-Right" | "A-S-C-Right"
        | "S-A-C-Right" => "\x1b[1;8C",
        "C-A-S-Up" | "C-S-A-Up" | "A-C-S-Up" | "S-C-A-Up" | "A-S-C-Up" | "S-A-C-Up" => "\x1b[1;8A",
        "C-A-S-Down" | "C-S-A-Down" | "A-C-S-Down" | "S-C-A-Down" | "A-S-C-Down" | "S-A-C-Down" => {
            "\x1b[1;8B"
        }

        k => {
            let chars: Vec<char> = k.chars().collect();

            match chars.as_slice() {
                ['C', '-', k @ 'a'..='z'] => {
                    s.push((*k as u8 - 0x60) as char);
                    &s
                }

                ['^', k @ 'a'..='z'] => {
                    s.push((*k as u8 - 0x60) as char);
                    &s
                }

                ['A', '-', k] => {
                    s.push('\x1b');
                    s.push(*k);
                    &s
                }

                _ => &key,
            }
        }
    }
    .to_owned()
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

        let command = parse(r#"{ "type": "sendKeys", "keys": ["Left"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[D"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["Right"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[C"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["Up"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[A"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["Down"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[B"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-Left"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;5D"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-Right"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;5C"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["S-Left"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;2D"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["S-Right"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;2C"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-Up"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;5A"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-Down"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;5B"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["S-Up"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;2A"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["S-Down"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;2B"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-Left"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;3D"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-Right"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;3C"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-Up"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;3A"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["A-Down"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;3B"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-S-Left"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;6D"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-S-Right"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;6C"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-S-Up"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;6A"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-S-Down"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;6B"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-A-Left"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;7D"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-A-Right"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;7C"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-A-Up"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;7A"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-A-Down"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;7B"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["S-A-Left"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;4D"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["S-A-Right"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;4C"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["S-A-Up"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;4A"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["S-A-Down"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;4B"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-A-S-Left"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;8D"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-A-S-Right"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;8C"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-A-S-Up"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;8A"));

        let command = parse(r#"{ "type": "sendKeys", "keys": ["C-A-S-Down"] }"#).unwrap();
        assert!(matches!(command, Command::Input(input) if input == "\x1b[1;8B"));

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
