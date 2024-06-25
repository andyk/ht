pub struct Vt {
    vt: avt::Vt,
}

impl Vt {
    pub fn new(cols: usize, rows: usize) -> Self {
        let vt = avt::Vt::builder().size(cols, rows).resizable(true).build();

        Self { vt }
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.vt.feed_str(&String::from_utf8_lossy(bytes));
    }

    pub fn get_text(&self) -> String {
        self.vt
            .lines()
            .iter()
            .map(|l| l.text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.vt.feed_str(&resize_seq(cols, rows));
    }

    pub fn cursor_key_app_mode(&self) -> bool {
        self.vt.arrow_key_app_mode()
    }
}

fn resize_seq(cols: usize, rows: usize) -> String {
    format!("\x1b[8;{};{}t", rows, cols)
}
