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

        Some("resize") => {
            let args: ResizeArgs = args_from_json_value(value)?;
            Ok(Command::Resize(args.cols, args.rows))
        }

        Some("getView") => Ok(Command::GetView),

        other => Err(format!("invalid command type: {other:?}")),
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
