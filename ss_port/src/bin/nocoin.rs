//! No-coin planner: finds the key presses of a run that never picks up a
//! coin, by searching the simulation itself.
//!
//! The simulation is deterministic (a seed and the key presses give the
//! run), so the planner needs no vision and no learning. It follows every
//! hero state the run can be in: every `period` frames each state is played
//! with each key (none, left, right, up, down); the states that crashed or
//! touched a coin are dropped, and so are the states already reached with
//! fewer stumbles and key presses (two heroes at the same place in the same
//! pose have the same future, whatever they did before). What is left at
//! the last frame is every way through the run; the one with the fewest
//! stumbles, then the fewest key presses, is written out.
//!
//! Following every state is too slow, so only `cap` are kept, picked to
//! cover every lane, pose and height. When that loses the way (every state
//! kept fails), the search goes back a few seconds and follows 4, 16, then
//! 64 times as many.
//!
//!   cargo run --release --bin nocoin -- --seed 1 --minutes 10 --out run.keys
//!
//! The output has one `frame key` line per key press; `ss_port --keys
//! run.keys` plays it back in the game, and `nocoin --check run.keys` plays
//! it without a window and reports the coins and the score.

use ss_port::game::{FrameInput, Game, GameState};
use ss_port::hero::Key;
use ss_port::sim_plugin::{DataPaths, KeyQueue, Sim};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

const ACTIONS: [Option<Key>; 5] = [None, Some(Key::Left), Some(Key::Right), Some(Key::Up), Some(Key::Down)];
const NAMES: [&str; 5] = ["-", "left", "right", "up", "down"];
/// Frames between two checkpoints.
const CHECKPOINT: i64 = 300;

struct Args {
    seed: i32,
    frames: i64,
    /// A key can be pressed every `period` frames.
    period: i64,
    /// States followed at most (the most varied are kept beyond that).
    cap: usize,
    /// Mystery boxes are allowed (their prize, opened after the run, can be coins).
    boxes: bool,
    out: PathBuf,
    quiet: bool,
}

fn args() -> Args {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let has = |k: &str| a.iter().any(|x| x == k);
    let minutes: f64 = get("--minutes").and_then(|v| v.parse().ok()).unwrap_or(10.0);
    Args {
        seed: get("--seed").and_then(|v| v.parse().ok()).unwrap_or(1),
        frames: get("--frames").and_then(|v| v.parse().ok()).unwrap_or((minutes * 3600.0) as i64),
        period: get("--period").and_then(|v| v.parse().ok()).unwrap_or(4),
        cap: get("--cap").and_then(|v| v.parse().ok()).unwrap_or(60),
        boxes: has("--allow-boxes"),
        out: get("--out").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("run.keys")),
        quiet: has("--quiet"),
    }
}

/// The run is over for a no-coin attempt.
fn failed(g: &Game, args: &Args) -> Option<&'static str> {
    if g.coins > 0 {
        Some("coin")
    } else if g.state == GameState::Gameover || g.hero.player.dead {
        Some("crash")
    } else if !args.boxes && !g.prizes.is_empty() {
        Some("mystery box")
    } else {
        None
    }
}

fn q(v: f64) -> i64 {
    (v * 1e4).round() as i64
}

/// Two states with the same signature in the same frame have the same
/// future (the level does not depend on what the hero did). Only what the
/// hero's next frames read goes in: fields left over from a finished lane
/// change or jump, and the camera, would make every history look new.
fn signature(g: &Game) -> u64 {
    let h = &g.hero;
    let c = h.body.center();
    let v = h.body.velocity();
    let mut s = DefaultHasher::new();
    (q(c.x), q(c.y), q(c.z), q(v.x), q(v.y), q(v.z), q(h.ground), h.fsm.current).hash(&mut s);
    let l = &h.lane;
    (l.lane, l.changing, l.queued_step, l.zapping).hash(&mut s);
    if l.changing {
        (l.last, q(l.start_x), q(l.end_x), q(l.duration), q(l.time), q(l.queued_duration)).hash(&mut s);
    }
    let j = &h.jump;
    (j.locked, j.is_jumping, j.is_double_jumping, j.going_upwards, j.should_jump_again).hash(&mut s);
    if j.is_jumping {
        (q(j.time), q(j.duration), q(j.start_y), q(j.end_y)).hash(&mut s);
    }
    let r = &h.roll;
    (r.locked, r.is_rolling, if r.is_rolling { q(r.time) } else { 0 }).hash(&mut s);
    let p = &h.player;
    (q(p.dizzy), q(p.bump_count), p.tunnel, g.prizes.len()).hash(&mut s);
    (format!("{:?}", g.guard.state), q(g.guard.distance)).hash(&mut s);
    // powerups: rare, so their whole state while one is on
    if h.pogo.is_on() || h.sneakers.is_on() || h.magnet.is_on() || h.multiplier.is_on() || h.jetpack.is_on() {
        format!("{:?}{:?}{:?}{:?}{:?}", h.pogo, h.sneakers, h.magnet, h.multiplier, h.jetpack).hash(&mut s);
    }
    s.finish()
}

/// A state the run can be in, and the key that led to it (`nodes` index).
#[derive(Clone)]
struct State {
    game: Game,
    node: u32,
    /// Stumbles so far, then key presses: the lower is kept among equals.
    cost: (u32, u32),
}

/// The keys worth pressing in this state. A lane key during a lane change
/// (turning back half way) is left out: it multiplies the places the hero
/// can be in without opening a way that pressing it a moment later does not.
fn keys(g: &Game) -> &'static [usize] {
    // keys do nothing before the run takes the keyboard
    if !(g.controller.enabled && g.state == GameState::Running) {
        &[0]
    } else if g.hero.lane.changing {
        &[0, 3, 4]
    } else {
        &[0, 1, 2, 3, 4]
    }
}

/// Over the cap: the states are grouped by where the hero is (lane, pose,
/// height, part of the lane change), and the groups give their cheapest
/// state in turn, so that no way of being on the track is lost for another.
fn thin(next: Vec<State>, cap: usize) -> Vec<State> {
    let place = |g: &Game| {
        let h = &g.hero;
        let pose = if h.roll.is_rolling {
            2
        } else if h.jump.is_jumping || h.body.bottom() > h.ground + 0.5 {
            1
        } else {
            0
        };
        let off = h.body.cx() - h.lane.lane as f64 * 20.0;
        (h.lane.lane, pose, (h.ground / 5.0).round() as i64, (h.body.bottom() / 4.0).round() as i64, h.body.velocity().y > 0.0, (off / 5.0).round() as i64)
    };
    let mut ranked: Vec<_> = next.into_iter().map(|s| (place(&s.game), s)).collect();
    ranked.sort_by_key(|(p, s)| (*p, s.cost));
    let mut rank = 0u32;
    let mut order: Vec<(u32, (u32, u32), State)> = Vec::with_capacity(ranked.len());
    let mut last = None;
    for (p, s) in ranked {
        rank = if last == Some(p) { rank + 1 } else { 0 };
        last = Some(p);
        order.push((rank, s.cost, s));
    }
    order.sort_by_key(|(rank, cost, _)| (*rank, *cost));
    order.truncate(cap);
    order.into_iter().map(|(_, _, s)| s).collect()
}

/// `--check FILE`: play a keys file the way the game does (`ss_port --keys`)
/// and report the run frame by frame: coins, crash, score.
fn check(root: &Path, file: &Path) {
    let driver = ss_port::sim_plugin::KeysDriver::load(file).expect("keys file");
    let header = std::fs::read_to_string(file).unwrap().lines().next().unwrap_or("").to_string();
    let word = |k: &str| header.split_whitespace().skip_while(|w| *w != k).nth(1).and_then(|v| v.parse::<i64>().ok());
    let (seed, frames) = (word("seed").expect("seed in the first line") as i32, word("frames").expect("frames in the first line"));
    let mut sim = Sim::new(&DataPaths::from_repo(root), seed, Some("S".into()), Box::new(driver)).expect("game data");
    sim.game.god = false;
    let last = sim.next_frame + frames - 1;
    let mut first_coin = None;
    while sim.next_frame <= last && sim.game.state != GameState::Gameover {
        sim.step();
        if sim.game.coins > 0 && first_coin.is_none() {
            first_coin = Some(sim.game.frame);
        }
    }
    let g = &sim.game;
    let alive = g.state == GameState::Running && !g.hero.player.dead;
    println!(
        "{}: seed {seed}, played to frame {} of {last} ({:.1} s), {}, {} coins{}, score {}",
        file.display(),
        g.frame,
        g.stats_time,
        if alive { "still running" } else { "crashed" },
        g.coins,
        first_coin.map(|f| format!(" (the first at frame {f})")).unwrap_or_default(),
        ss_port::flow::score(g)
    );
    if !(alive && g.coins == 0 && g.frame == last) {
        std::process::exit(1);
    }
}

fn main() {
    let args = args();
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
    if let Some(file) = std::env::args().skip_while(|a| a != "--check").nth(1) {
        return check(root, Path::new(&file));
    }
    let sim = Sim::new(&DataPaths::from_repo(root), args.seed, Some("S".into()), Box::new(KeyQueue::default())).expect("game data");
    let Sim { mut game, next_frame, .. } = sim;
    game.god = false;
    let first = next_frame;
    let last = first + args.frames - 1;
    let debug = std::env::var("NOCOIN_DEBUG").is_ok();

    // nodes[i] = (parent, frame, key): the key pressed in `frame`
    let mut nodes: Vec<(u32, i64, u8)> = vec![(u32::MAX, first - 1, 0)];
    let mut states = vec![State { game, node: 0, cost: (0, 0) }];
    let mut causes: HashMap<&'static str, u64> = HashMap::new();
    let (mut steps, mut most, mut thinned) = (0u64, 0usize, 0u32);
    let started = Instant::now();
    let mut dead_end = None;
    // the states of a few seconds ago, to go back to from a dead end
    let mut checkpoints: VecDeque<(i64, Vec<State>)> = VecDeque::new();
    // after a dead end: 4^level times the cap until frame `wide_to`
    let (mut level, mut wide_to, mut rewinds) = (0u32, 0i64, 0u32);

    let mut f = first;
    while f <= last {
        if f > wide_to {
            level = 0;
        }
        let cap = args.cap << (2 * level);
        let period = if level >= 2 { 1 } else { args.period };
        if level == 0 && checkpoints.back().is_none_or(|c| f - c.0 >= CHECKPOINT) {
            checkpoints.push_back((f, states.clone()));
            if checkpoints.len() > 8 {
                checkpoints.pop_front();
            }
        }
        let to = (f + period - 1).min(last);
        // the cheapest first: among equal states the first is kept
        states.sort_by_key(|s| s.cost);
        let mut next: Vec<State> = Vec::with_capacity(states.len() * 3);
        let mut seen: HashSet<u64> = HashSet::with_capacity(states.len() * 3);
        let mut dropped: Vec<&'static str> = Vec::new();
        for st in &states {
            'key: for &a in keys(&st.game) {
                let mut g = st.game.clone();
                let bumps = g.hero.player.bump_count;
                let mut stumbles = 0;
                for frame in f..=to {
                    let key = if frame == f { ACTIONS[a] } else { None };
                    g.step(frame, FrameInput { keys: key.into_iter().collect(), nav: Vec::new() });
                    g.log.clear();
                    steps += 1;
                    if let Some(why) = failed(&g, &args) {
                        dropped.push(why);
                        continue 'key;
                    }
                    stumbles = stumbles.max((g.hero.player.bump_count > bumps) as u32);
                }
                if !seen.insert(signature(&g)) {
                    continue;
                }
                nodes.push((st.node, f, a as u8));
                next.push(State { game: g, node: nodes.len() as u32 - 1, cost: (st.cost.0 + stumbles, st.cost.1 + (a != 0) as u32) });
            }
        }
        for c in &dropped {
            *causes.entry(c).or_default() += 1;
        }
        if next.is_empty() {
            // every state kept fails: the way through was thinned out
            // earlier. Back a few seconds, with more states (and then with
            // a key every frame); further back and wider if that fails too
            let wider = if to <= wide_to { level + 1 } else { 1 };
            let back = to - CHECKPOINT * 2 * wider as i64;
            let at = checkpoints.iter().rposition(|c| c.0 <= back).unwrap_or(0);
            if wider > 3 || checkpoints.is_empty() {
                dead_end = Some((to, dropped));
                break;
            }
            checkpoints.truncate(at + 1);
            let (frame, kept) = checkpoints.back().unwrap();
            if !args.quiet {
                eprintln!("dead end at frame {to} ({:?}): back to frame {frame} with {} states", dropped.first().unwrap(), args.cap << (2 * wider));
            }
            (f, states) = (*frame, kept.clone());
            (level, wide_to) = (wider, to + CHECKPOINT);
            rewinds += 1;
            continue;
        }
        most = most.max(next.len());
        if next.len() > cap {
            thinned += 1;
            next = thin(next, cap);
        }
        states = next;
        f = to + 1;
        let played = f - first;
        if debug || (!args.quiet && level == 0 && played % 3600 < period && played >= 3600) {
            let g = &states[0].game;
            eprintln!(
                "{:5.1} min  frame {to}  {} states  score {}  speed {:.0}  {} steps  {:.1} s",
                played as f64 / 3600.0,
                states.len(),
                ss_port::flow::score(g),
                -g.hero.body.velocity().z * 60.0,
                steps,
                started.elapsed().as_secs_f64()
            );
        }
    }

    // the run behind the cheapest state left (at a dead end: the last one alive)
    let best = states.iter().min_by_key(|s| s.cost).unwrap();
    let mut keys = Vec::new();
    let mut node = best.node;
    while node != 0 {
        let (parent, frame, a) = nodes[node as usize];
        if a != 0 {
            keys.push((frame, a as usize));
        }
        node = parent;
    }
    keys.reverse();
    let mut out = std::fs::File::create(&args.out).expect("output file");
    writeln!(out, "# seed {} first {} frames {} complete {}", args.seed, first, args.frames, dead_end.is_none()).unwrap();
    for (f, a) in &keys {
        writeln!(out, "{f} {}", NAMES[*a]).unwrap();
    }

    let secs = started.elapsed().as_secs_f64();
    let search = format!("search {secs:.1} s, {steps} steps, {most} states at most, thinned {thinned} times, {rewinds} dead ends, dropped {causes:?}");
    match dead_end {
        None => {
            let g = &best.game;
            println!(
                "seed {}: {} frames ({:.1} min) with {} coins, score {}, {} key presses, {} stumbles, {} ways left; {search}",
                args.seed,
                args.frames,
                args.frames as f64 / 3600.0,
                g.coins,
                ss_port::flow::score(g),
                keys.len(),
                best.cost.0,
                states.len()
            );
            println!("keys written to {}", args.out.display());
        }
        Some((t, dropped)) => {
            let mut why: HashMap<&str, usize> = HashMap::new();
            for d in dropped {
                *why.entry(d).or_default() += 1;
            }
            if debug {
                describe(&best.game);
            }
            println!("seed {}: dead end at frame {t} ({:.1} s of run): the {} states left all fail ({why:?}); {search}", args.seed, (t - first) as f64 / 60.0, states.len());
            println!("keys up to there written to {}", args.out.display());
            std::process::exit(1);
        }
    }
}

/// The hero and the level entities around it (NOCOIN_DEBUG).
fn describe(g: &Game) {
    let h = &g.hero;
    let c = h.body.center();
    eprintln!(
        "hero lane {} x {:.1} y {:.1} z {:.1} ground {:.1} pose {} speed {:.0} powerups: magnet {} jetpack {} sneakers {} pogo {}",
        h.lane.lane,
        c.x,
        c.y,
        c.z,
        h.ground,
        h.fsm.current,
        -h.body.velocity().z * 60.0,
        h.magnet.is_on(),
        h.jetpack.is_on(),
        h.sneakers.is_on(),
        h.pogo.is_on()
    );
    let mut near: Vec<(i64, String)> = g
        .level_entities
        .iter()
        .filter_map(|&id| {
            let e = g.ent(id);
            let b = e.body.as_ref()?;
            let (p, s) = (b.center(), b.size());
            let ahead = c.z - p.z;
            (ahead > -40.0 - s.z / 2.0 && ahead < 400.0).then(|| ((ahead * 10.0) as i64, format!("  {:6.1} ahead  x {:5.1}  y {:4.1}  size {:2.0} x {:2.0} x {:3.0}  {:?}", ahead, p.x, p.y, s.x, s.y, s.z, e.cls)))
        })
        .collect();
    near.sort();
    for (_, l) in near {
        eprintln!("{l}");
    }
}
