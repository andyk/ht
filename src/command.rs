#[derive(Debug)]
pub enum Command {
    Input(Vec<InputSeq>),
    Snapshot,
    Resize(usize, usize),
}

#[derive(Debug, PartialEq)]
pub enum InputSeq {
    Standard(String),
    Cursor(String, String),
}

pub fn seqs_to_bytes(seqs: &[InputSeq], app_mode: bool) -> Vec<u8> {
    let mut bytes = Vec::new();

    for seq in seqs {
        bytes.extend_from_slice(seq_as_bytes(seq, app_mode));
    }

    bytes
}

fn seq_as_bytes(seq: &InputSeq, app_mode: bool) -> &[u8] {
    match (seq, app_mode) {
        (InputSeq::Standard(seq), _) => seq.as_bytes(),
        (InputSeq::Cursor(seq1, _seq2), false) => seq1.as_bytes(),
        (InputSeq::Cursor(_seq1, seq2), true) => seq2.as_bytes(),
    }
}
