//! The coinless challenge has one life: `flow::decline_revive` leaves Save me
//! (and its "not enough keys" popup) for the results.

use ss_port::flow::Screen;
use ss_port::sim_plugin::{DataPaths, KeyQueue, Sim};
use std::path::Path;

#[test]
fn decline_revive_leaves_save_me() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
    let mut s = Sim::new(&DataPaths::from_repo(root), 1, Some("S".into()), Box::new(KeyQueue::default())).unwrap();
    s.game.god = false;
    // no key: the run ends on the first train (frame 185), Save me opens at 231
    while !matches!(s.game.flow.screen, Screen::SaveMe { .. }) {
        s.step();
        assert!(s.game.frame < 400);
    }
    ss_port::flow::decline_revive(&mut s.game);
    assert!(!matches!(s.game.flow.screen, Screen::SaveMe { .. } | Screen::NotEnough { .. }));
}
