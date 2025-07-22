use anyhow::Result;
use futures_util::{stream, Stream, StreamExt};
use serde_json::json;
use std::future;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

pub struct Session {
    vt: avt::Vt,
    broadcast_tx: broadcast::Sender<Event>,
    stream_time: f64,
    start_time: Instant,
    last_event_time: Instant,
    pid: i32,
}

#[derive(Clone)]
pub enum Event {
    Init(f64, usize, usize, i32, String, String),
    Output(f64, String),
    Resize(f64, usize, usize),
    Snapshot(usize, usize, String, String),
}

pub struct Client(oneshot::Sender<Subscription>);

pub struct Subscription {
    init: Event,
    broadcast_rx: broadcast::Receiver<Event>,
}

impl Session {
    pub fn new(cols: usize, rows: usize, pid: i32) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1024);
        let now = Instant::now();

        Self {
            vt: build_vt(cols, rows),
            broadcast_tx,
            stream_time: 0.0,
            start_time: now,
            last_event_time: now,
            pid,
        }
    }

    pub fn output(&mut self, data: String) {
        self.vt.feed_str(&data);
        let time = self.start_time.elapsed().as_secs_f64();
        let _ = self.broadcast_tx.send(Event::Output(time, data));
        self.stream_time = time;
        self.last_event_time = Instant::now();
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        resize_vt(&mut self.vt, cols, rows);
        let time = self.start_time.elapsed().as_secs_f64();
        let _ = self.broadcast_tx.send(Event::Resize(time, cols, rows));
        self.stream_time = time;
        self.last_event_time = Instant::now();
    }

    pub fn snapshot(&self) {
        let (cols, rows) = self.vt.size();

        let _ = self.broadcast_tx.send(Event::Snapshot(
            cols,
            rows,
            self.vt.dump(),
            self.text_view(),
        ));
    }

    pub fn cursor_key_app_mode(&self) -> bool {
        self.vt.cursor_key_app_mode()
    }

    pub fn subscribe(&self) -> Subscription {
        let (cols, rows) = self.vt.size();

        let init = Event::Init(
            self.elapsed_time(),
            cols,
            rows,
            self.pid,
            self.vt.dump(),
            self.text_view(),
        );

        let broadcast_rx = self.broadcast_tx.subscribe();

        Subscription { init, broadcast_rx }
    }

    fn elapsed_time(&self) -> f64 {
        self.stream_time + self.last_event_time.elapsed().as_secs_f64()
    }

    fn text_view(&self) -> String {
        self.vt
            .view()
            .iter()
            .map(|l| l.text())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Event {
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Event::Init(_time, cols, rows, pid, seq, text) => json!({
                "type": "init",
                "data": json!({
                    "cols": cols,
                    "rows": rows,
                    "pid": pid,
                    "seq": seq,
                    "text": text,
                })
            }),

            Event::Output(_time, seq) => json!({
                "type": "output",
                "data": json!({
                    "seq": seq
                })
            }),

            Event::Resize(_time, cols, rows) => json!({
                "type": "resize",
                "data": json!({
                    "cols": cols,
                    "rows": rows,
                })
            }),

            Event::Snapshot(cols, rows, seq, text) => json!({
                "type": "snapshot",
                "data": json!({
                    "cols": cols,
                    "rows": rows,
                    "seq": seq,
                    "text": text,
                })
            }),
        }
    }
}

fn build_vt(cols: usize, rows: usize) -> avt::Vt {
    avt::Vt::builder().size(cols, rows).build()
}

fn resize_vt(vt: &mut avt::Vt, cols: usize, rows: usize) {
    vt.resize(cols, rows);
}

impl Client {
    pub fn accept(self, subscription: Subscription) {
        let _ = self.0.send(subscription);
    }
}

pub async fn stream(
    clients_tx: &mpsc::Sender<Client>,
) -> Result<impl Stream<Item = Result<Event, BroadcastStreamRecvError>>> {
    let (sub_tx, sub_rx) = oneshot::channel();
    clients_tx.send(Client(sub_tx)).await?;
    let sub = tokio::time::timeout(Duration::from_secs(5), sub_rx).await??;
    let init = stream::once(future::ready(Ok(sub.init)));
    let events = BroadcastStream::new(sub.broadcast_rx);

    Ok(init.chain(events))
}
