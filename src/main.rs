use std::collections::HashSet;
use std::num::Wrapping;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use rayon::prelude::*;
use rusqlite::{params, Connection};

// ─── Constants (matching types.ts) ───

const SALT: &str = "friend-2026-401";

const SPECIES: &[&str] = &[
    "duck", "goose", "blob", "cat", "dragon", "octopus", "owl", "penguin",
    "turtle", "snail", "ghost", "axolotl", "capybara", "cactus", "robot",
    "rabbit", "mushroom", "chonk",
];

const RARITIES: &[&str] = &["common", "uncommon", "rare", "epic", "legendary"];
const RARITY_WEIGHTS: &[u32] = &[60, 25, 10, 4, 1];
const RARITY_STARS: &[&str] = &["★", "★★", "★★★", "★★★★", "★★★★★"];

const EYES: &[&str] = &["·", "✦", "×", "◉", "@", "°"];

const HATS: &[&str] = &[
    "none", "crown", "tophat", "propeller", "halo", "wizard", "beanie", "tinyduck",
];

const STAT_NAMES: &[&str] = &["DEBUGGING", "PATIENCE", "CHAOS", "WISDOM", "SNARK"];

const RARITY_FLOOR: &[i32] = &[5, 15, 25, 35, 50];

// ─── Sprite templates (matching sprites.ts, frame 0 only) ───

const HAT_LINES: &[&str] = &[
    "",              // none
    "   \\^^^/    ", // crown
    "   [___]    ",  // tophat
    "    -+-     ",  // propeller
    "   (   )    ",  // halo
    "    /^\\     ", // wizard
    "   (___)    ",  // beanie
    "    ,>      ",  // tinyduck
];

fn get_body(species: &str) -> &'static [&'static str] {
    match species {
        "duck" => &[
            "            ",
            "    __      ",
            "  <({E} )___  ",
            "   (  ._>   ",
            "    `--´    ",
        ],
        "goose" => &[
            "            ",
            "     ({E}>    ",
            "     ||     ",
            "   _(__)_   ",
            "    ^^^^    ",
        ],
        "blob" => &[
            "            ",
            "   .----.   ",
            "  ( {E}  {E} )  ",
            "  (      )  ",
            "   `----´   ",
        ],
        "cat" => &[
            "            ",
            "   /\\_/\\    ",
            "  ( {E}   {E})  ",
            "  (  ω  )   ",
            "  (\")_(\")",
        ],
        "dragon" => &[
            "            ",
            "  /^\\  /^\\  ",
            " <  {E}  {E}  > ",
            " (   ~~   ) ",
            "  `-vvvv-´  ",
        ],
        "octopus" => &[
            "            ",
            "   .----.   ",
            "  ( {E}  {E} )  ",
            "  (______)  ",
            "  /\\/\\/\\/\\  ",
        ],
        "owl" => &[
            "            ",
            "   /\\  /\\   ",
            "  (({E})({E}))  ",
            "  (  ><  )  ",
            "   `----´   ",
        ],
        "penguin" => &[
            "            ",
            "  .---.     ",
            "  ({E}>{E})     ",
            " /(   )\\    ",
            "  `---´     ",
        ],
        "turtle" => &[
            "            ",
            "   _,--._   ",
            "  ( {E}  {E} )  ",
            " /[______]\\ ",
            "  ``    ``  ",
        ],
        "snail" => &[
            "            ",
            " {E}    .--.  ",
            "  \\  ( @ )  ",
            "   \\_`--´   ",
            "  ~~~~~~~   ",
        ],
        "ghost" => &[
            "            ",
            "   .----.   ",
            "  / {E}  {E} \\  ",
            "  |      |  ",
            "  ~`~``~`~  ",
        ],
        "axolotl" => &[
            "            ",
            "}~(______)~{",
            "}~({E} .. {E})~{",
            "  ( .--. )  ",
            "  (_/  \\_)  ",
        ],
        "capybara" => &[
            "            ",
            "  n______n  ",
            " ( {E}    {E} ) ",
            " (   oo   ) ",
            "  `------´  ",
        ],
        "cactus" => &[
            "            ",
            " n  ____  n ",
            " | |{E}  {E}| | ",
            " |_|    |_| ",
            "   |    |   ",
        ],
        "robot" => &[
            "            ",
            "   .[||].   ",
            "  [ {E}  {E} ]  ",
            "  [ ==== ]  ",
            "  `------´  ",
        ],
        "rabbit" => &[
            "            ",
            "   (\\__/)   ",
            "  ( {E}  {E} )  ",
            " =(  ..  )= ",
            "  (\")__(\")",
        ],
        "mushroom" => &[
            "            ",
            " .-o-OO-o-. ",
            "(__________)",
            "   |{E}  {E}|   ",
            "   |____|   ",
        ],
        "chonk" => &[
            "            ",
            "  /\\    /\\  ",
            " ( {E}    {E} ) ",
            " (   ..   ) ",
            "  `------´  ",
        ],
        _ => &["            "; 5],
    }
}

fn render_sprite(species: &str, eye: &str, hat: &str) -> String {
    let body = get_body(species);
    let mut lines: Vec<String> = body.iter().map(|l| l.replace("{E}", eye)).collect();

    // Hat replacement: only if line 0 is blank and hat != none
    let hat_idx = HATS.iter().position(|&h| h == hat).unwrap_or(0);
    if hat != "none" && lines[0].trim().is_empty() {
        lines[0] = HAT_LINES[hat_idx].to_string();
    }

    // Drop blank line 0 if no hat (matches sprites.ts logic)
    if lines[0].trim().is_empty() {
        lines.remove(0);
    }

    lines.join("\n")
}

fn render_card(roll: &BuddyRoll) -> String {
    let rarity_idx = RARITIES.iter().position(|&r| r == roll.rarity).unwrap_or(0);
    let stars = RARITY_STARS[rarity_idx];
    let species_upper = roll.species.to_uppercase();

    // Header line: "  ★★★ RARE                  MUSHROOM  "
    let header_left = format!("  {} {}", stars, roll.rarity.to_uppercase());
    let header_right = format!("{}  ", species_upper);
    let padding = 38usize.saturating_sub(header_left.chars().count() + header_right.chars().count());
    let header = format!("{}{:>width$}", header_left, header_right, width = padding + header_right.len());

    let sprite_lines: Vec<&str> = roll.sprite.split('\n').collect();

    // Stats bars
    let stat_values = [roll.debugging, roll.patience, roll.chaos, roll.wisdom, roll.snark];
    let stat_lines: Vec<String> = STAT_NAMES.iter().zip(stat_values.iter()).map(|(name, &val)| {
        let filled = (val as usize) / 10;
        let empty = 10 - filled;
        format!("  {:<10} {}{} {:>3}",
            name,
            "█".repeat(filled),
            "░".repeat(empty),
            val
        )
    }).collect();

    let mut card = String::new();
    let w = 38; // inner width
    let border = |s: &str| format!("│{:<width$}│", s, width = w);
    let empty_line = border(&" ".repeat(w));

    card.push_str(&format!("╭{}╮\n", "─".repeat(w)));
    card.push_str(&empty_line); card.push('\n');
    card.push_str(&border(&header)); card.push('\n');
    card.push_str(&empty_line); card.push('\n');

    // Sprite
    for sl in &sprite_lines {
        let padded = format!("  {:<width$}", sl, width = w - 2);
        // Truncate to card width if needed
        card.push_str(&border(&padded[..padded.len().min(w)])); card.push('\n');
    }

    card.push_str(&empty_line); card.push('\n');
    card.push_str(&border(&format!("  (unnamed)"))); card.push('\n');
    card.push_str(&empty_line); card.push('\n');

    // Stats
    for sl in &stat_lines {
        card.push_str(&border(sl)); card.push('\n');
    }

    card.push_str(&empty_line); card.push('\n');
    card.push_str(&format!("╰{}╯", "─".repeat(w)));

    card
}

// ─── Core algorithms (matching companion.ts) ───

fn hash_string(s: &str) -> u32 {
    let mut h = Wrapping(2166136261u32);
    for &b in s.as_bytes() {
        h ^= Wrapping(b as u32);
        h *= Wrapping(16777619u32);
    }
    h.0
}

struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6d2b79f5);
        let a = self.state;
        let t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        let t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        let t = t ^ (t >> 14);
        (t as f64) / 4294967296.0
    }
}

fn pick(rng: &mut Mulberry32, arr: &[&str]) -> usize {
    (rng.next_f64() * arr.len() as f64) as usize
}

fn roll_rarity(rng: &mut Mulberry32) -> usize {
    let mut roll = rng.next_f64() * 100.0;
    for (i, &w) in RARITY_WEIGHTS.iter().enumerate() {
        roll -= w as f64;
        if roll < 0.0 {
            return i;
        }
    }
    0
}

#[derive(Debug, Clone)]
struct BuddyRoll {
    user_id: String,
    species: &'static str,
    rarity: &'static str,
    eye: &'static str,
    hat: &'static str,
    shiny: bool,
    debugging: i32,
    patience: i32,
    chaos: i32,
    wisdom: i32,
    snark: i32,
    sprite: String,
}

fn roll_buddy(user_id: &str) -> BuddyRoll {
    let key = format!("{}{}", user_id, SALT);
    let seed = hash_string(&key);
    let mut rng = Mulberry32::new(seed);

    let rarity_idx = roll_rarity(&mut rng);
    let rarity = RARITIES[rarity_idx];
    let species_idx = pick(&mut rng, SPECIES);
    let species = SPECIES[species_idx];
    let eye_idx = pick(&mut rng, EYES);
    let eye = EYES[eye_idx];
    let hat = if rarity_idx == 0 {
        "none"
    } else {
        HATS[pick(&mut rng, HATS)]
    };
    let shiny = rng.next_f64() < 0.01;

    // rollStats
    let floor = RARITY_FLOOR[rarity_idx];
    let peak_idx = (rng.next_f64() * STAT_NAMES.len() as f64) as usize;
    let mut dump_idx = (rng.next_f64() * STAT_NAMES.len() as f64) as usize;
    while dump_idx == peak_idx {
        dump_idx = (rng.next_f64() * STAT_NAMES.len() as f64) as usize;
    }

    let mut stats = [0i32; 5];
    for i in 0..5 {
        if i == peak_idx {
            stats[i] = 100i32.min(floor + 50 + (rng.next_f64() * 30.0) as i32);
        } else if i == dump_idx {
            stats[i] = 1i32.max(floor - 10 + (rng.next_f64() * 15.0) as i32);
        } else {
            stats[i] = floor + (rng.next_f64() * 40.0) as i32;
        }
    }

    let sprite = render_sprite(species, eye, hat);

    BuddyRoll {
        user_id: user_id.to_string(),
        species,
        rarity,
        eye,
        hat,
        shiny,
        debugging: stats[0],
        patience: stats[1],
        chaos: stats[2],
        wisdom: stats[3],
        snark: stats[4],
        sprite,
    }
}

// ─── Database ───

fn init_db(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS buddies (
            user_id      TEXT PRIMARY KEY,
            species      TEXT NOT NULL,
            rarity       TEXT NOT NULL,
            eye          TEXT NOT NULL,
            hat          TEXT NOT NULL,
            shiny        INTEGER NOT NULL,
            debugging    INTEGER NOT NULL,
            patience     INTEGER NOT NULL,
            chaos        INTEGER NOT NULL,
            wisdom       INTEGER NOT NULL,
            snark        INTEGER NOT NULL,
            sprite       TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_appearance ON buddies(species, rarity, hat, eye, shiny);
        CREATE INDEX IF NOT EXISTS idx_rarity ON buddies(rarity, species);
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;",
    )
    .expect("Failed to init DB");
}

fn insert_buddy(conn: &Connection, r: &BuddyRoll) {
    conn.execute(
        "INSERT OR IGNORE INTO buddies VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            r.user_id, r.species, r.rarity, r.eye, r.hat,
            r.shiny as i32,
            r.debugging, r.patience, r.chaos, r.wisdom, r.snark,
            r.sprite,
        ],
    )
    .ok();
}

// ─── Coverage tracking ───

/// Count unique (species, rarity) combos found so far
fn count_phase1_coverage(conn: &Connection) -> usize {
    conn.query_row(
        "SELECT COUNT(DISTINCT species || '|' || rarity) FROM buddies",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// Count unique (species, rarity, hat) combos — phase 2
fn count_phase2_coverage(conn: &Connection) -> usize {
    conn.query_row(
        "SELECT COUNT(DISTINCT species || '|' || rarity || '|' || hat) FROM buddies",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// Count unique (species, rarity, hat, eye) combos — phase 3
fn count_phase3_coverage(conn: &Connection) -> usize {
    conn.query_row(
        "SELECT COUNT(DISTINCT species || '|' || rarity || '|' || hat || '|' || eye) FROM buddies",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

fn gen_random_user_id(rng: &mut impl Rng) -> String {
    let bytes: [u8; 32] = rng.gen();
    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

// ─── Main ───

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("bruteforce");

    match mode {
        "verify" => run_verify(),
        "bruteforce" => {
            let max_iters: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1_000_000);
            run_bruteforce(max_iters);
        }
        "query" => {
            let species = args.get(2).map(|s| s.as_str());
            let rarity = args.get(3).map(|s| s.as_str());
            run_query(species, rarity);
        }
        "stats" => run_stats(),
        _ => {
            eprintln!("用法:");
            eprintln!("  buddy_bruteforce verify              验证两个已知 case");
            eprintln!("  buddy_bruteforce bruteforce [N]      撞库 N 次（默认 100万）");
            eprintln!("  buddy_bruteforce query [species] [rarity]  查询结果");
            eprintln!("  buddy_bruteforce stats               显示覆盖统计");
        }
    }
}

fn run_verify() {
    let cases: &[(&str, &str, &str, &str, &str, bool, [i32; 5])] = &[
        (
            "5490d80131ba79823a9b9333feeac01ab8d2b10bccbf2699415320cdb65927c0",
            "common", "capybara", "·", "none", false,
            [12, 38, 32, 5, 83],
        ),
        (
            "fcee8ee31b8f9f96943ca18472e2267b289a2c3ab755d27529289721177d7e45",
            "rare", "mushroom", "◉", "tinyduck", false,
            [40, 18, 35, 100, 42],
        ),
    ];

    let mut all_pass = true;
    for (uid, exp_rarity, exp_species, exp_eye, exp_hat, exp_shiny, exp_stats) in cases {
        let r = roll_buddy(uid);
        let stats = [r.debugging, r.patience, r.chaos, r.wisdom, r.snark];
        let pass = r.rarity == *exp_rarity
            && r.species == *exp_species
            && r.eye == *exp_eye
            && r.hat == *exp_hat
            && r.shiny == *exp_shiny
            && stats == *exp_stats;

        if pass {
            println!("✅ PASS: {} {} eye={} hat={}", r.rarity, r.species, r.eye, r.hat);
        } else {
            println!("❌ FAIL: uid={}...", &uid[..16]);
            println!("  期望: {} {} eye={} hat={} shiny={} stats={:?}", exp_rarity, exp_species, exp_eye, exp_hat, exp_shiny, exp_stats);
            println!("  实际: {} {} eye={} hat={} shiny={} stats={:?}", r.rarity, r.species, r.eye, r.hat, r.shiny, stats);
            all_pass = false;
        }
    }

    if all_pass {
        println!("\n🎉 验证通过！");
    } else {
        std::process::exit(1);
    }
}

/// Compact roll result for in-memory storage (no sprite — render on write)
struct RollCompact {
    user_id: String,
    species_idx: u8,
    rarity_idx: u8,
    eye_idx: u8,
    hat_idx: u8,
    shiny: bool,
    stats: [i32; 5],
}

fn roll_compact(user_id: &str) -> RollCompact {
    let key = format!("{}{}", user_id, SALT);
    let seed = hash_string(&key);
    let mut rng = Mulberry32::new(seed);

    let rarity_idx = roll_rarity(&mut rng) as u8;
    let species_idx = pick(&mut rng, SPECIES) as u8;
    let eye_idx = pick(&mut rng, EYES) as u8;
    let hat_idx = if rarity_idx == 0 {
        0 // "none"
    } else {
        pick(&mut rng, HATS) as u8
    };
    let shiny = rng.next_f64() < 0.01;

    let floor = RARITY_FLOOR[rarity_idx as usize];
    let peak = (rng.next_f64() * STAT_NAMES.len() as f64) as usize;
    let mut dump = (rng.next_f64() * STAT_NAMES.len() as f64) as usize;
    while dump == peak {
        dump = (rng.next_f64() * STAT_NAMES.len() as f64) as usize;
    }
    let mut stats = [0i32; 5];
    for i in 0..5 {
        if i == peak {
            stats[i] = 100i32.min(floor + 50 + (rng.next_f64() * 30.0) as i32);
        } else if i == dump {
            stats[i] = 1i32.max(floor - 10 + (rng.next_f64() * 15.0) as i32);
        } else {
            stats[i] = floor + (rng.next_f64() * 40.0) as i32;
        }
    }

    RollCompact { user_id: user_id.to_string(), species_idx, rarity_idx, eye_idx, hat_idx, shiny, stats }
}

/// Coverage key: (species_idx, rarity_idx, hat_idx, eye_idx) packed into u32
fn coverage_key(r: &RollCompact) -> u32 {
    (r.species_idx as u32) << 24
        | (r.rarity_idx as u32) << 16
        | (r.hat_idx as u32) << 8
        | (r.eye_idx as u32)
}

fn coverage_key_p1(r: &RollCompact) -> u16 {
    (r.species_idx as u16) << 8 | (r.rarity_idx as u16)
}

fn coverage_key_p2(r: &RollCompact) -> u32 {
    (r.species_idx as u32) << 16 | (r.rarity_idx as u32) << 8 | (r.hat_idx as u32)
}

/// P4 key: appearance + shiny, packed into u64
fn coverage_key_p4(r: &RollCompact) -> u64 {
    (coverage_key(r) as u64) << 1 | (r.shiny as u64)
}

fn flush_to_db(conn: &Connection, results: &[RollCompact]) {
    conn.execute_batch("BEGIN").unwrap();
    {
        let mut stmt = conn
            .prepare_cached("INSERT OR IGNORE INTO buddies VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)")
            .unwrap();
        for r in results {
            let species = SPECIES[r.species_idx as usize];
            let rarity = RARITIES[r.rarity_idx as usize];
            let eye = EYES[r.eye_idx as usize];
            let hat = HATS[r.hat_idx as usize];
            let sprite = render_sprite(species, eye, hat);
            stmt.execute(params![
                r.user_id, species, rarity, eye, hat,
                r.shiny as i32,
                r.stats[0], r.stats[1], r.stats[2], r.stats[3], r.stats[4],
                sprite,
            ]).ok();
        }
    }
    conn.execute_batch("COMMIT").unwrap();
}

fn run_bruteforce(max_iters: u64) {
    let db_path = std::env::current_dir().unwrap().join("buddies.db");
    println!("数据库: {}", db_path.display());
    println!("线程数: {}", rayon::current_num_threads());

    let conn = Connection::open(&db_path).expect("Failed to open DB");
    init_db(&conn);

    let phase1_target: usize = 18 * 5;
    let phase2_target: usize = 18 + 18 * 4 * 8;
    let phase3_target: usize = 108 + 3456; // 3564
    let phase4_target: usize = phase3_target * 2; // 7128 (each combo × shiny/not)

    // Load existing coverage from DB
    let mut seen_p1: HashSet<u16> = HashSet::new();
    let mut seen_p2: HashSet<u32> = HashSet::new();
    let mut seen_p3: HashSet<u32> = HashSet::new();
    let mut seen_p4: HashSet<u64> = HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT species, rarity, hat, eye, shiny FROM buddies").unwrap();
        let rows = stmt.query_map([], |row| {
            let sp: String = row.get(0)?;
            let ra: String = row.get(1)?;
            let ha: String = row.get(2)?;
            let ey: String = row.get(3)?;
            let sh: i32 = row.get(4)?;
            let si = SPECIES.iter().position(|&s| s == sp).unwrap_or(0) as u8;
            let ri = RARITIES.iter().position(|&r| r == ra).unwrap_or(0) as u8;
            let hi = HATS.iter().position(|&h| h == ha).unwrap_or(0) as u8;
            let ei = EYES.iter().position(|&e| e == ey).unwrap_or(0) as u8;
            Ok((si, ri, hi, ei, sh != 0))
        }).unwrap();
        for row in rows {
            let (si, ri, hi, ei, shiny) = row.unwrap();
            seen_p1.insert((si as u16) << 8 | (ri as u16));
            seen_p2.insert((si as u32) << 16 | (ri as u32) << 8 | (hi as u32));
            let k3 = (si as u32) << 24 | (ri as u32) << 16 | (hi as u32) << 8 | (ei as u32);
            seen_p3.insert(k3);
            seen_p4.insert((k3 as u64) << 1 | (shiny as u64));
        }
    }

    println!("已有覆盖: P1 {}/{}, P2 {}/{}, P3 {}/{}, P4 {}/{}",
        seen_p1.len(), phase1_target, seen_p2.len(), phase2_target,
        seen_p3.len(), phase3_target, seen_p4.len(), phase4_target);

    if seen_p4.len() >= phase4_target {
        println!("已全部覆盖（含 shiny），无需继续！");
        return;
    }

    let pb = ProgressBar::new(max_iters);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}) | {msg}"
        )
        .unwrap()
        .progress_chars("█▓░"),
    );

    let start = Instant::now();
    let shiny_count = AtomicU64::new(0);
    let progress = AtomicU64::new(0);
    let seen_p1 = Mutex::new(seen_p1);
    let seen_p2 = Mutex::new(seen_p2);
    let seen_p3 = Mutex::new(seen_p3);
    let seen_p4 = Mutex::new(seen_p4);
    let results = Mutex::new(Vec::<RollCompact>::new());
    let done = AtomicU64::new(0); // 1 = early exit

    // Process in parallel chunks
    let chunk_size: u64 = 10_000;
    let num_chunks = (max_iters + chunk_size - 1) / chunk_size;

    (0..num_chunks).into_par_iter().for_each(|chunk_idx| {
        if done.load(Ordering::Relaxed) == 1 {
            return;
        }

        let chunk_start = chunk_idx * chunk_size;
        let chunk_end = max_iters.min(chunk_start + chunk_size);
        let mut rng = rand::thread_rng();
        let mut local_results: Vec<RollCompact> = Vec::new();
        let mut local_shiny: u64 = 0;

        for i in chunk_start..chunk_end {
            let uid = gen_random_user_id(&mut rng);
            let r = roll_compact(&uid);

            if r.shiny { local_shiny += 1; }

            let k3 = coverage_key(&r);
            // Quick check without lock
            local_results.push(r);

            if i % 10000 == 0 && done.load(Ordering::Relaxed) == 1 {
                break;
            }
        }

        shiny_count.fetch_add(local_shiny, Ordering::Relaxed);

        // Merge into global state
        {
            let mut gp1 = seen_p1.lock().unwrap();
            let mut gp2 = seen_p2.lock().unwrap();
            let mut gp3 = seen_p3.lock().unwrap();
            let mut gp4 = seen_p4.lock().unwrap();
            let mut gres = results.lock().unwrap();

            for r in local_results {
                let k4 = coverage_key_p4(&r);
                if !gp4.contains(&k4) {
                    gp1.insert(coverage_key_p1(&r));
                    gp2.insert(coverage_key_p2(&r));
                    gp3.insert(coverage_key(&r));
                    gp4.insert(k4);
                    gres.push(r);
                }
            }

            let p = progress.fetch_add(chunk_size, Ordering::Relaxed) + chunk_size;
            pb.set_position(p.min(max_iters));
            pb.set_message(format!(
                "P3:{}/3564 P4:{}/7128 new:{}",
                gp3.len(), gp4.len(), gres.len()
            ));

            if gp4.len() >= phase4_target {
                done.store(1, Ordering::Relaxed);
            }
        }
    });

    pb.finish_and_clear();
    let compute_elapsed = start.elapsed();

    let results = results.into_inner().unwrap();
    let final_p1 = seen_p1.into_inner().unwrap().len();
    let final_p2 = seen_p2.into_inner().unwrap().len();
    let final_p3 = seen_p3.into_inner().unwrap().len();
    let final_p4 = seen_p4.into_inner().unwrap().len();

    // Flush to SQLite
    println!("\n写入 SQLite... ({} 条记录)", results.len());
    let write_start = Instant::now();
    for chunk in results.chunks(10000) {
        flush_to_db(&conn, chunk);
    }
    let write_elapsed = write_start.elapsed();

    let total_rows: u64 = conn
        .query_row("SELECT COUNT(*) FROM buddies", [], |r| r.get(0))
        .unwrap_or(0);
    let db_shiny: u64 = conn
        .query_row("SELECT COUNT(*) FROM buddies WHERE shiny=1", [], |r| r.get(0))
        .unwrap_or(0);

    println!("\n═══ 撞库完成 ═══");
    println!("计算耗时:   {:.2}s ({} 线程)", compute_elapsed.as_secs_f64(), rayon::current_num_threads());
    println!("写入耗时:   {:.2}s", write_elapsed.as_secs_f64());
    println!("总迭代:     {}", max_iters);
    println!("速度:       {:.0}/s", max_iters as f64 / compute_elapsed.as_secs_f64());
    println!("新增记录:   {}", results.len());
    println!("数据库总行: {}", total_rows);
    println!("本轮闪光:   {} | 数据库闪光: {}", shiny_count.load(Ordering::Relaxed), db_shiny);
    println!("───────────────");
    println!("阶段一覆盖: {}/{} (物种×稀有度)", final_p1, phase1_target);
    println!("阶段二覆盖: {}/{} (物种×稀有度×帽子)", final_p2, phase2_target);
    println!("阶段三覆盖: {}/{} (物种×稀有度×帽子×眼睛)", final_p3, phase3_target);
    println!("阶段四覆盖: {}/{} (全外观×shiny)", final_p4, phase4_target);
}

fn run_query(species: Option<&str>, rarity: Option<&str>) {
    let db_path = std::env::current_dir().unwrap().join("buddies.db");
    let conn = Connection::open(&db_path).expect("Failed to open DB");

    let (sql, values): (String, Vec<String>) = match (species, rarity) {
        (Some(s), Some(r)) => (
            "SELECT * FROM buddies WHERE species=?1 AND rarity=?2 LIMIT 20".into(),
            vec![s.to_string(), r.to_string()],
        ),
        (Some(s), None) => (
            "SELECT * FROM buddies WHERE species=?1 ORDER BY CASE rarity WHEN 'legendary' THEN 0 WHEN 'epic' THEN 1 WHEN 'rare' THEN 2 WHEN 'uncommon' THEN 3 ELSE 4 END LIMIT 20".into(),
            vec![s.to_string()],
        ),
        _ => (
            "SELECT * FROM buddies ORDER BY CASE rarity WHEN 'legendary' THEN 0 WHEN 'epic' THEN 1 WHEN 'rare' THEN 2 WHEN 'uncommon' THEN 3 ELSE 4 END LIMIT 20".into(),
            vec![],
        ),
    };

    let mut stmt = conn.prepare(&sql).unwrap();
    let rows = match values.len() {
        2 => stmt.query_map(params![values[0], values[1]], map_row),
        1 => stmt.query_map(params![values[0]], map_row),
        _ => stmt.query_map([], map_row),
    }
    .unwrap();

    let mut count = 0;
    for row in rows {
        let r = row.unwrap();
        println!("{}", render_card(&r));
        println!("  userID: {}", r.user_id);
        println!();
        count += 1;
    }

    if count == 0 {
        println!("未找到匹配结果。");
    } else {
        println!("共 {} 条结果", count);
    }
}

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<BuddyRoll> {
    Ok(BuddyRoll {
        user_id: row.get(0)?,
        species: leak_str(row.get::<_, String>(1)?),
        rarity: leak_str(row.get::<_, String>(2)?),
        eye: leak_str(row.get::<_, String>(3)?),
        hat: leak_str(row.get::<_, String>(4)?),
        shiny: row.get::<_, i32>(5)? != 0,
        debugging: row.get(6)?,
        patience: row.get(7)?,
        chaos: row.get(8)?,
        wisdom: row.get(9)?,
        snark: row.get(10)?,
        sprite: row.get(11)?,
    })
}

/// Leak small strings to get &'static str — fine for query results
fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn run_stats() {
    let db_path = std::env::current_dir().unwrap().join("buddies.db");
    let conn = Connection::open(&db_path).expect("Failed to open DB");

    let total: u64 = conn.query_row("SELECT COUNT(*) FROM buddies", [], |r| r.get(0)).unwrap_or(0);
    let shiny: u64 = conn.query_row("SELECT COUNT(*) FROM buddies WHERE shiny=1", [], |r| r.get(0)).unwrap_or(0);
    let p1 = count_phase1_coverage(&conn);
    let p2 = count_phase2_coverage(&conn);
    let p3 = count_phase3_coverage(&conn);

    println!("═══ 数据库统计 ═══");
    println!("总行数:     {}", total);
    println!("闪光数:     {}", shiny);
    println!("阶段一:     {}/90  (物种×稀有度)", p1);
    println!("阶段二:     {}/594 (物种×稀有度×帽子)", p2);
    println!("阶段三:     {}/3564(物种×稀有度×帽子×眼睛)", p3);

    let p4: usize = conn.query_row(
        "SELECT COUNT(DISTINCT species || '|' || rarity || '|' || hat || '|' || eye || '|' || shiny) FROM buddies",
        [], |r| r.get(0),
    ).unwrap_or(0);
    println!("阶段四:     {}/7128(全外观×shiny)", p4);

    println!("\n─── 按稀有度分布 ───");
    let mut stmt = conn.prepare("SELECT rarity, COUNT(*) FROM buddies GROUP BY rarity ORDER BY CASE rarity WHEN 'legendary' THEN 0 WHEN 'epic' THEN 1 WHEN 'rare' THEN 2 WHEN 'uncommon' THEN 3 ELSE 4 END").unwrap();
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
    }).unwrap();
    for row in rows {
        let (rarity, count) = row.unwrap();
        println!("  {:<12} {}", rarity, count);
    }

    println!("\n─── 按物种分布 ───");
    let mut stmt = conn.prepare("SELECT species, COUNT(*) FROM buddies GROUP BY species ORDER BY COUNT(*) DESC").unwrap();
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
    }).unwrap();
    for row in rows {
        let (species, count) = row.unwrap();
        println!("  {:<12} {}", species, count);
    }
}
