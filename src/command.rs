#[derive(Debug)]
pub enum Command {
    Input(String),
    GetView,
    Resize(usize, usize),
}

pub fn parse(line: &str) -> Result<Command, String> {
    serde_json::from_str::<serde_json::Value>(line)
        .map_err(|e| format!("JSON parse error: {e}"))
        .and_then(build_command)
}

fn build_command(json: serde_json::Value) -> Result<Command, String> {
    match json["type"].as_str() {
        Some("input") => {
            let payload = json["payload"]
                .as_str()
                .ok_or("payload missing".to_string())?
                .to_string();

            Ok(Command::Input(payload))
        }

        Some("resize") => {
            let cols = json["cols"].as_u64().ok_or("cols missing".to_string())?;
            let rows = json["rows"].as_u64().ok_or("rows missing".to_string())?;

            Ok(Command::Resize(cols as usize, rows as usize))
        }

        Some("getView") => Ok(Command::GetView),

        other => Err(format!("invalid command type: {other:?}")),
    }
}
