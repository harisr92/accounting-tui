//! Termion has no poll API, so input and ticks come from background threads over a bounded channel

use std::io;
use std::sync::mpsc::{self, Receiver, RecvError, SyncSender};
use std::thread;
use std::time::Duration;

use termion::event::Key;
use termion::input::TermRead;

const CHANNEL_CAPACITY: usize = 64;

pub enum Event {
    Key(Key),
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
        for key in io::stdin().keys().map_while(Result::ok) {
            if tx.send(Event::Key(key)).is_err() {
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
