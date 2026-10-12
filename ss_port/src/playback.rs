//! Video-style controls of a run played from a keys file (`--keys`): speed
//! and seeking.
//!
//! The run is deterministic, so any frame can be reached again: the app
//! keeps a copy of the game every 20 seconds ([`Snapshots`]) and seeking
//! goes to the last copy before the frame, then steps to it.
//!
//! The controls are plain atomics: the keyboard sets them natively, the
//! page's buttons through the web build's exports (`main.rs`).

use crate::game::Game;
use crate::sim_plugin::Sim;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering::Relaxed};

/// Speed in percent (100 = real time).
pub static SPEED: AtomicU32 = AtomicU32::new(100);
pub static PAUSED: AtomicBool = AtomicBool::new(false);
/// The frame asked for (-1: none).
pub static SEEK: AtomicI64 = AtomicI64::new(-1);
/// Where the run is, and its first and last frames (for the page's bar).
pub static FRAME: AtomicI64 = AtomicI64::new(0);
pub static FIRST: AtomicI64 = AtomicI64::new(0);
pub static LAST: AtomicI64 = AtomicI64::new(0);

/// Frames between two copies of the game.
const EVERY: i64 = 1200;

/// Copies of the game: (the next frame to play, the game), in frame order.
#[derive(Default)]
pub struct Snapshots(Vec<(i64, Game)>);

impl Snapshots {
    /// Before a step: keep a copy when one is due.
    pub fn keep(&mut self, sim: &Sim) {
        if self.0.last().is_none_or(|(f, _)| sim.next_frame >= f + EVERY) {
            self.0.push((sim.next_frame, sim.game.clone()));
        }
    }

    /// Bring `sim` to `frame` (the next frame to play).
    pub fn seek(&mut self, sim: &mut Sim, frame: i64) {
        let frame = frame.max(self.0.first().map_or(0, |s| s.0));
        // the last copy before the frame, unless playing on from here is shorter
        if let Some((f, g)) = self.0.iter().rev().find(|(f, _)| *f <= frame) {
            if frame < sim.next_frame || *f > sim.next_frame {
                sim.restore(g.clone(), *f);
            }
        }
        while sim.next_frame < frame {
            self.keep(sim);
            sim.step();
            sim.forget_log();
        }
        // the sounds of the frames skipped
        sim.game.sound.cmds.clear();
    }
}

/// The speed as a factor, 0 while paused.
pub fn speed() -> f32 {
    if PAUSED.load(Relaxed) { 0.0 } else { SPEED.load(Relaxed) as f32 / 100.0 }
}

/// The coinless challenge (`--coinless`): a coin ends the run. What the
/// page shows: [score, seconds run, 1 while running, 1 once a coin was touched].
pub static CHALLENGE: [AtomicI64; 4] = [AtomicI64::new(0), AtomicI64::new(0), AtomicI64::new(0), AtomicI64::new(0)];
/// The game's Settings panel is open (the page adds its Keys row to it).
pub static SETTINGS: AtomicBool = AtomicBool::new(false);

/// The challenge's run on record: its seed and the keys pressed (frame,
/// key letter), so that a saved run can be played again.
pub static RECORD: std::sync::Mutex<(i32, Vec<(i64, char)>)> = std::sync::Mutex::new((0, Vec::new()));

/// What the page's end-of-run panel asks for: 1 back to the title, 2 a new run at once.
pub static NEXT: AtomicI64 = AtomicI64::new(0);
