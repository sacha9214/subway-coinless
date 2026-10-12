//! The coinless challenge's end of run: no Save me, straight on to the
//! results, and Space starts the next run.

use ss_port::flow::Screen;
use ss_port::game::GameState;
use ss_port::hero::Key;
use ss_port::sim_plugin::{DataPaths, KeyQueue, Sim};
use std::path::Path;

#[test]
fn no_revive_then_play_again() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
    let keys = KeyQueue::default();
    let mut s = Sim::new(&DataPaths::from_repo(root), 1, Some("S".into()), Box::new(keys.clone())).unwrap();
    s.game.god = false;
    let mut seen = Vec::new();
    // no key: the run ends on the first train (frame 185); Space now and then, as a player would
    for f in 0..1200 {
        if f % 60 == 59 && s.game.state == GameState::Gameover {
            keys.0.borrow_mut().push(Key::Action);
        }
        s.step();
        ss_port::flow::decline_revive(&mut s.game);
        let name = format!("{:?}", s.game.flow.screen).split([' ', '{', '(']).next().unwrap().to_string();
        if seen.last() != Some(&name) {
            seen.push(name);
        }
        assert!(!matches!(s.game.flow.screen, Screen::SaveMe { .. } | Screen::NotEnough { .. }), "Save me showed");
        if s.game.run_starts >= 2 {
            break;
        }
    }
    assert_eq!(s.game.run_starts, 2, "a second run started; screens: {seen:?}");
    assert!(seen.contains(&"Results".to_string()), "screens: {seen:?}");
}
