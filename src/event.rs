//! Input and ticks come from background threads over a bounded channel

use std::sync::mpsc::{self, Receiver, RecvError, SyncSender};
use std::thread;
use std::time::Duration;

use ratatui::crossterm::event::{self as term, Event as TermEvent};

use crate::ui::key::Key;

const CHANNEL_CAPACITY: usize = 64;

pub enum Event {
    Key(Key),
    Resize,
    Tick,
}

pub struct Events {
    rx: Receiver<Event>,
}

impl Events {
    pub fn spawn(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::sync_channel(CHANNEL_CAPACITY);
        spawn_input(tx.clone());
        spawn_ticker(tx, tick_rate);
        Self { rx }
    }

    pub fn next(&self) -> Result<Event, RecvError> {
        self.rx.recv()
    }
}

fn spawn_input(tx: SyncSender<Event>) {
    thread::spawn(move || {
        while let Ok(event) = term::read() {
            let event = match event {
                TermEvent::Key(key) => match Key::from_event(key) {
                    Some(key) => Event::Key(key),
                    None => continue,
                },
                TermEvent::Resize(..) => Event::Resize,
                _ => continue,
            };
            if tx.send(event).is_err() {
                return;
            }
        }
    });
}

fn spawn_ticker(tx: SyncSender<Event>, tick_rate: Duration) {
    thread::spawn(move || {
        loop {
            thread::sleep(tick_rate);
            // A full queue means the app is busy; dropping a tick is harmless
            if let Err(mpsc::TrySendError::Disconnected(_)) = tx.try_send(Event::Tick) {
                return;
            }
        }
    });
}
