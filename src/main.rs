fn main() {
    for line in std::io::stdin().lines() {
        let mut vt = avt::Vt::builder().size(80, 24).build();
        let json: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();

        match &json["action"].as_str() {
            Some("input") => {
                // pty.input();
            }

            Some("getView") => {
                let text = vt
                    .lines()
                    .iter()
                    .map(|l| l.text())
                    .collect::<Vec<_>>()
                    .join("\n");

                let resp = serde_json::json!({ "view": text });
                println!("{}", serde_json::to_string(&resp).unwrap());
            }

            _ => (),
        }
    }
}
