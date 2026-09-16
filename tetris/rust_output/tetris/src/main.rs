// Generated from Poly source code
#![allow(
    unused_variables,
    unused_mut,
    unused_imports,
    dead_code,
    unused_parens,
    unreachable_patterns
)]

const COLS: i32 = 10;
const ROWS: i32 = 20;
const BUF: i32 = 2;
const TOTAL: i32 = 22;
const CELL: i32 = 30;
const PREV: i32 = 20;
const DAS_DELAY: i32 = 170;
const DAS_RATE: i32 = 35;
const FADE: i32 = 180;
const WINW: i32 = 660;
const WINH: i32 = 732;
const BX: i32 = 180;
const BY: i32 = 66;
const BW: i32 = 300;
const BH: i32 = 600;
const PW: i32 = 100;
const PX0: i32 = 62;
const PX1: i32 = 498;
const BG: i32 = 0x05010A;
const GRIDBG: i32 = 0x000814;
const GRIDLN: i32 = 0x26204A;
const PANELB: i32 = 0x2A1C63;
const FLASH: i32 = 0x888888;
const WHITE: i32 = 0xFFFFFF;
const DIM: i32 = 0x8A8A8A;
const RED: i32 = 0xFF3A4A;
const CYNE: i32 = 0x2BF2FF;
const YELLOW: i32 = 0xFFE74A;

struct Piece {
    t: i32,
    rot: i32,
    row: i32,
    col: i32,
}

struct Bag {
    q: Vec<i32>,
    nq: Vec<i32>,
    p: i32,
    q_peek: i32,
}

struct GameState {
    grid: Vec<i32>,
    q_hit: bool,
    bag: Bag,
    hold: i32,
    can_hold: bool,
    cur: Piece,
    score: i32,
    lines: i32,
    level: i32,
    game_over: bool,
    paused: bool,
    started: bool,
    soft: bool,
    accum: i64,
    gms: i64,
    high: i32,
    das_l: i64,
    das_r: i64,
    last_t: i64,
    fx_a: i32,
    fx_t: i64,
    fx_cells: Vec<i32>,
    seed: u64,
    q_ghost_row: i32,
    q_scored: i32,
    q_queue: Vec<i32>,
}

impl Bag {
    fn shuffled(mut self, seed: u64) -> (Bag, u64) {
        let mut arr: Vec<i32> = vec![0, 1, 2, 3, 4, 5, 6];
        let mut i: i32 = 6;
        let mut s: u64 = seed;
        while (i > 0) {
            s = (s ^ (s << 13));
            s = (s ^ (s >> 7));
            s = (s ^ (s << 17));
            let mut j: i32 = (((s >> 33) as i32) % i);
            let mut tmp: i32 = arr[(i) as usize];
            arr[(i) as usize] = arr[(j) as usize];
            arr[(j) as usize] = tmp;
            i = (i - 1);
        }
        self.q = arr;
        return (self, s);
    }
    fn refill(mut self, seed: u64) -> (Bag, i32, u64) {
        let mut b = self;
        let mut s: u64 = seed;
        if (b.p >= (b.q.len() as i32)) {
            let mut qv: Vec<i32> = b.q;
            let mut nqv: Vec<i32> = b.nq;
            let mut pv: i32 = b.p;
            let mut nb = Bag {
                q: nqv,
                nq: qv,
                p: 0,
                q_peek: 0,
            };
            let mut pair = nb.shuffled(s);
            nb = pair.0;
            s = pair.1;
            let mut q2: Vec<i32> = nb.q;
            let mut nq2: Vec<i32> = nb.nq;
            b = Bag {
                q: nq2,
                nq: q2,
                p: 0,
                q_peek: 0,
            };
        }

        let mut v: i32 = b.q[(b.p) as usize];
        b.p = (b.p + 1);
        return (b, v, s);
    }
    fn peek(mut self, off: i32) -> Bag {
        let mut idx: i32 = (self.p + off);
        if (idx < (self.q.len() as i32)) {
            self.q_peek = self.q[(idx) as usize];
        } else {
            if (idx < ((self.q.len() as i32) + (self.nq.len() as i32))) {
                self.q_peek = self.nq[(idx - (self.q.len() as i32)) as usize];
            } else {
                self.q_peek = 0;
            }
        }

        return self;
    }
}

impl GameState {
    fn spawn_piece(mut self, forced: i32) -> GameState {
        let mut t: i32 = forced;
        if (t < 0) {
            let mut r = self.bag.refill(self.seed);
            self.bag = r.0;
            self.seed = r.2;
            t = r.1;
        }

        let mut w: i32 = piece_w(shape_bits((t * 4)));
        self.cur = Piece {
            t: t,
            rot: 0,
            row: BUF,
            col: ((COLS - w) / 2),
        };
        return self;
    }
    fn collides(mut self, t: i32, rot: i32, row: i32, col: i32) -> GameState {
        let mut bits: i32 = shape_bits(((t * 4) + rot));
        self.q_hit = false;
        for r in 0..=3 {
            for c in 0..=3 {
                if bit_at(bits, r, c) {
                    let mut rr: i32 = (row + r);
                    let mut cc: i32 = (col + c);
                    if ((cc < 0) || (cc >= COLS)) {
                        self.q_hit = true;
                    }

                    if (rr >= TOTAL) {
                        self.q_hit = true;
                    }

                    if ((rr >= 0) && (rr < TOTAL)) {
                        if (self.grid[((rr * COLS) + cc) as usize] > 0) {
                            self.q_hit = true;
                        }
                    }
                }
            }
        }
        return self;
    }
    fn move_h(mut self, dx: i32) -> GameState {
        let mut t: i32 = self.cur.t;
        let mut rot: i32 = self.cur.rot;
        let mut row: i32 = self.cur.row;
        let mut col: i32 = self.cur.col;
        self = self.collides(t, rot, row, (col + dx));
        if (!self.q_hit) {
            self.cur.col = (self.cur.col + dx);
            audio_play(0)
        }

        return self;
    }
    fn rotate(mut self, dir: i32) -> GameState {
        let mut nr: i32 = (((self.cur.rot + dir) + 4) % 4);
        let mut kicks: Vec<i32> = vec![0, (-1), 1, (-2), 2];
        let mut t: i32 = self.cur.t;
        let mut row: i32 = self.cur.row;
        let mut col: i32 = self.cur.col;
        for k in kicks.iter().cloned() {
            self = self.collides(t, nr, row, (col + k));
            if (!self.q_hit) {
                self.cur.col = (self.cur.col + k);
                self.cur.rot = nr;
                audio_play(1);
                return self;
            }
        }
        return self;
    }
    fn hard_drop(mut self) -> GameState {
        let mut d: i32 = 0;
        let mut t: i32 = self.cur.t;
        let mut rot: i32 = self.cur.rot;
        let mut col: i32 = self.cur.col;
        let mut row: i32 = self.cur.row;
        self = self.collides(t, rot, (row + 1), col);
        while (!self.q_hit) {
            self = self.collides(t, rot, (row + 1), col);
            if (!self.q_hit) {
                row = (row + 1);
                d = (d + 1);
            }
        }
        self.cur.row = row;
        self.score = (self.score + (d * 2));
        audio_play(6);
        self = self.lock();
        return self;
    }
    fn do_hold(mut self) -> GameState {
        if (!self.can_hold) {
            audio_play(0);
            return self;
        }

        let mut cur: i32 = self.cur.t;
        if (self.hold < 0) {
            self.hold = cur;
            self = self.spawn_piece((-1));
        } else {
            let mut sw: i32 = self.hold;
            self.hold = cur;
            self = self.spawn_piece(sw);
        }

        self.can_hold = false;
        audio_play(1);
        return self;
    }
    fn toggle_pause(mut self) -> GameState {
        if (self.started && (!self.game_over)) {
            self.paused = (!self.paused);
        }

        return self;
    }
    fn toggle_mute(mut self) -> GameState {
        audio_play(8);
        return self;
    }
    fn restart(mut self) -> GameState {
        let mut hi: i32 = self.high;
        let mut sd: u64 = self.seed;
        let mut fresh = make_game(sd, hi);
        return fresh;
    }
    fn lock(mut self) -> GameState {
        let mut bits: i32 = shape_bits(((self.cur.t * 4) + self.cur.rot));
        for r in 0..=3 {
            for c in 0..=3 {
                if bit_at(bits, r, c) {
                    let mut rr: i32 = (self.cur.row + r);
                    let mut cc: i32 = (self.cur.col + c);
                    if ((rr >= 0) && (rr < TOTAL)) {
                        self.grid[((rr * COLS) + cc) as usize] = (self.cur.t + 1);
                    }
                }
            }
        }
        audio_play(2);
        self.can_hold = true;
        self = self.clear_lines();
        self = self.spawn_piece((-1));
        let mut ct: i32 = self.cur.t;
        let mut crot: i32 = self.cur.rot;
        let mut crow: i32 = self.cur.row;
        let mut ccol: i32 = self.cur.col;
        self = self.collides(ct, crot, crow, ccol);
        if self.q_hit {
            self.game_over = true;
            self.started = false;
            self.paused = false;
            audio_play(7);
            if (self.score > self.high) {
                self.high = self.score;
                highscore_save(self.score)
            }
        }

        return self;
    }
    fn clear_lines(mut self) -> GameState {
        let mut full: Vec<i32> = vec![];
        let mut r: i32 = BUF;
        while (r <= (TOTAL - 1)) {
            let mut all: bool = true;
            let mut c: i32 = 0;
            while (c <= (COLS - 1)) {
                if (self.grid[((r * COLS) + c) as usize] == 0) {
                    all = false;
                }

                c = (c + 1);
            }
            if all {
                full.push(r)
            }

            r = (r + 1);
        }
        if ((full.len() as i32) == 0) {
            return self;
        }

        let mut n: i32 = (full.len() as i32);
        if (n == 1) {
            self.score = (self.score + (40 * (self.level + 1)));
        } else {
            if (n == 2) {
                self.score = (self.score + (100 * (self.level + 1)));
            } else {
                if (n == 3) {
                    self.score = (self.score + (300 * (self.level + 1)));
                } else {
                    self.score = (self.score + (800 * (self.level + 1)));
                }
            }
        }

        self.lines = (self.lines + n);
        let mut nl: i32 = (self.lines / 10);
        if (nl > self.level) {
            self.level = nl;
            self.gms = gravity_ms(self.level);
        }

        if (n == 4) {
            audio_play(4)
        } else {
            audio_play(3)
        }

        self.fx_a = n;
        self.fx_t = now_ms();
        let mut fcells: Vec<i32> = vec![];
        for f in full.iter().cloned() {
            for c in 0..=(COLS - 1) {
                fcells.push(self.grid[((f * COLS) + c) as usize]);
            }
        }
        self.fx_cells = fcells;
        let mut keep: Vec<i32> = vec![];
        let mut r: i32 = BUF;
        while (r <= (TOTAL - 1)) {
            let mut is_full: bool = false;
            for f in full.iter().cloned() {
                if (f == r) {
                    is_full = true;
                }
            }
            if (!is_full) {
                for c in 0..=(COLS - 1) {
                    keep.push(self.grid[((r * COLS) + c) as usize]);
                }
            }

            r = (r + 1);
        }
        let mut ng: Vec<i32> = vec![];
        for i in 0..=((BUF * COLS) - 1) {
            ng.push(0);
        }
        let mut empty_rows: i32 = (ROWS - ((keep.len() as i32) / COLS));
        for i in 0..=((empty_rows * COLS) - 1) {
            ng.push(0);
        }
        for v in keep.iter().cloned() {
            ng.push(v);
        }
        self.grid = ng;
        return self;
    }
    fn gravity(mut self, dt: i64) -> GameState {
        if (((!self.started) || self.paused) || self.game_over) {
            return self;
        }

        let mut stepms: i64 = self.gms;
        if self.soft {
            stepms = (stepms / 12);
            if (stepms < 20) {
                stepms = 20;
            }
        }

        self.accum = (self.accum + dt);
        while (self.accum >= stepms) {
            self.accum = (self.accum - stepms);
            let mut gt: i32 = self.cur.t;
            let mut grot: i32 = self.cur.rot;
            let mut grow: i32 = self.cur.row;
            let mut gcol: i32 = self.cur.col;
            self = self.collides(gt, grot, (grow + 1), gcol);
            if self.q_hit {
                self.accum = 0;
                self = self.lock();
                return self;
            }

            self.cur.row = (self.cur.row + 1);
            if self.soft {
                self.score = (self.score + 1);
            }
        }
        return self;
    }
    fn query_drop(mut self) -> GameState {
        let mut row: i32 = self.cur.row;
        let mut t: i32 = self.cur.t;
        let mut rot: i32 = self.cur.rot;
        let mut col: i32 = self.cur.col;
        self = self.collides(t, rot, (row + 1), col);
        while (!self.q_hit) {
            row = (row + 1);
            self = self.collides(t, rot, (row + 1), col);
        }
        self.q_ghost_row = row;
        return self;
    }
    fn query_cell(mut self, row: i32, col: i32) -> GameState {
        self.q_scored = 0;
        if ((((row >= 0) && (row < TOTAL)) && (col >= 0)) && (col < COLS)) {
            self.q_scored = self.grid[((row * COLS) + col) as usize];
        }

        return self;
    }
    fn query_queue(mut self) -> GameState {
        let mut b = self.bag;
        let mut out: Vec<i32> = vec![];
        for i in 0..=4 {
            b = b.peek(i);
            out.push(b.q_peek);
        }
        self.bag = b;
        self.q_queue = out;
        return self;
    }
}

fn shape_bits(kr: i32) -> i32 {
    match kr {
        0 => {
            return 15;
        }
        1 => {
            return 4369;
        }
        2 => {
            return 15;
        }
        3 => {
            return 4369;
        }
        4 => {
            return 51;
        }
        5 => {
            return 51;
        }
        6 => {
            return 51;
        }
        7 => {
            return 51;
        }
        8 => {
            return 114;
        }
        9 => {
            return 305;
        }
        10 => {
            return 39;
        }
        11 => {
            return 562;
        }
        12 => {
            return 54;
        }
        13 => {
            return 561;
        }
        14 => {
            return 54;
        }
        15 => {
            return 561;
        }
        16 => {
            return 99;
        }
        17 => {
            return 306;
        }
        18 => {
            return 99;
        }
        19 => {
            return 306;
        }
        20 => {
            return 113;
        }
        21 => {
            return 275;
        }
        22 => {
            return 71;
        }
        23 => {
            return 802;
        }
        24 => {
            return 116;
        }
        25 => {
            return 785;
        }
        26 => {
            return 23;
        }
        _ => {
            return 547;
        }
    }
}

fn bit_at(bits: i32, r: i32, c: i32) -> bool {
    return (((bits >> ((r * 4) + c)) & 1) == 1);
}

fn piece_w(bits: i32) -> i32 {
    let mut w: i32 = 0;
    for r in 0..=3 {
        for c in 0..=3 {
            if bit_at(bits, r, c) {
                if ((c + 1) > w) {
                    w = (c + 1);
                }
            }
        }
    }
    return w;
}

fn piece_h(bits: i32) -> i32 {
    let mut h: i32 = 0;
    for r in 0..=3 {
        for c in 0..=3 {
            if bit_at(bits, r, c) {
                if ((r + 1) > h) {
                    h = (r + 1);
                }
            }
        }
    }
    return h;
}

fn color_of(t: i32) -> i32 {
    match t {
        0 => {
            return 0x2BF2FF;
        }
        1 => {
            return 0xFFE74A;
        }
        2 => {
            return 0xC54BFF;
        }
        3 => {
            return 0x66FF6A;
        }
        4 => {
            return 0xFF3A4A;
        }
        5 => {
            return 0x4A7BFF;
        }
        _ => {
            return 0xFF9A3A;
        }
    }
}

fn gravity_ms(level: i32) -> i64 {
    let mut lv: i32 = level;
    if (lv > 29) {
        lv = 29;
    }

    let mut f: i32 = 1;
    match lv {
        0 => {
            f = 48;
        }
        1 => {
            f = 43;
        }
        2 => {
            f = 38;
        }
        3 => {
            f = 33;
        }
        4 => {
            f = 28;
        }
        5 => {
            f = 23;
        }
        6 => {
            f = 18;
        }
        7 => {
            f = 13;
        }
        8 => {
            f = 8;
        }
        9 => {
            f = 6;
        }
        10 => {
            f = 5;
        }
        11 => {
            f = 5;
        }
        12 => {
            f = 5;
        }
        13 => {
            f = 4;
        }
        14 => {
            f = 4;
        }
        15 => {
            f = 4;
        }
        16 => {
            f = 3;
        }
        17 => {
            f = 3;
        }
        18 => {
            f = 3;
        }
        29 => {
            f = 1;
        }
        _ => {
            f = 2;
        }
    }
    return ((f as i64 * 1000) / 60);
}

fn glyph(ch: i32) -> u64 {
    match ch {
        48 => {
            return 15623448110;
        }
        49 => {
            return 15170932932;
        }
        50 => {
            return 33357578798;
        }
        51 => {
            return 15619854623;
        }
        52 => {
            return 8891181448;
        }
        53 => {
            return 15620127807;
        }
        54 => {
            return 15621129292;
        }
        55 => {
            return 2216829471;
        }
        56 => {
            return 15621113390;
        }
        57 => {
            return 6728664622;
        }
        65 => {
            return 18842895918;
        }
        66 => {
            return 16694887983;
        }
        67 => {
            return 15603893806;
        }
        68 => {
            return 7836583207;
        }
        69 => {
            return 33321092159;
        }
        70 => {
            return 1108837439;
        }
        71 => {
            return 32801457710;
        }
        72 => {
            return 18842895921;
        }
        73 => {
            return 15170932878;
        }
        74 => {
            return 6753100060;
        }
        75 => {
            return 18560947505;
        }
        76 => {
            return 33320633377;
        }
        77 => {
            return 18842572657;
        }
        78 => {
            return 18842703473;
        }
        79 => {
            return 15621211694;
        }
        80 => {
            return 1108854319;
        }
        81 => {
            return 23946905134;
        }
        82 => {
            return 18561353263;
        }
        83 => {
            return 16660235326;
        }
        84 => {
            return 4433514655;
        }
        85 => {
            return 15621211697;
        }
        86 => {
            return 4648912433;
        }
        87 => {
            return 19182306865;
        }
        88 => {
            return 18834663985;
        }
        89 => {
            return 4433521201;
        }
        90 => {
            return 33321787935;
        }
        33 => {
            return 4299296900;
        }
        47 => {
            return 1143087376;
        }
        45 => {
            return 1015808;
        }
        46 => {
            return 6643777536;
        }
        60 => {
            return 8726284424;
        }
        62 => {
            return 2290622594;
        }
        58 => {
            return 207624384;
        }
        _ => {
            return 0;
        }
    }
}

fn pad(n: i32, w: i32) -> String {
    let mut s = format!("{:?}", n);
    while ((s.len() as i32) < w) {
        s = format!("{}{}", String::from("0"), s);
    }
    return s;
}

fn fill_cell(fb0: Vec<i32>, x0: i32, y0: i32, sz: i32, col: i32, alpha: i32) -> Vec<i32> {
    let mut fb: Vec<i32> = fb0;
    let mut yy: i32 = y0;
    while (yy <= ((y0 + sz) - 1)) {
        let mut xx: i32 = x0;
        while (xx <= ((x0 + sz) - 1)) {
            if ((((xx >= 0) && (xx < WINW)) && (yy >= 0)) && (yy < WINH)) {
                let mut idx: i32 = ((yy * WINW) + xx);
                if (alpha == 1) {
                    fb[(idx) as usize] = col;
                } else {
                    let mut base: i32 = fb[(idx) as usize];
                    let mut br: i32 = ((base >> 16) & 255);
                    let mut bg: i32 = ((base >> 8) & 255);
                    let mut bb: i32 = (base & 255);
                    let mut cr: i32 = ((col >> 16) & 255);
                    let mut cg: i32 = ((col >> 8) & 255);
                    let mut cb: i32 = (col & 255);
                    let mut nr: i32 = (((br * 4) + cr) / 5);
                    let mut ng: i32 = (((bg * 4) + cg) / 5);
                    let mut nb: i32 = (((bb * 4) + cb) / 5);
                    fb[(idx) as usize] = (((nr << 16) | (ng << 8)) | nb);
                }
            }

            xx = (xx + 1);
        }
        yy = (yy + 1);
    }
    return fb;
}

fn fill_rect(fb0: Vec<i32>, x0: i32, y0: i32, w: i32, h: i32, col: i32) -> Vec<i32> {
    let mut fb: Vec<i32> = fb0;
    let mut yy: i32 = y0;
    while (yy <= ((y0 + h) - 1)) {
        let mut xx: i32 = x0;
        while (xx <= ((x0 + w) - 1)) {
            if ((((xx >= 0) && (xx < WINW)) && (yy >= 0)) && (yy < WINH)) {
                fb[((yy * WINW) + xx) as usize] = col;
            }

            xx = (xx + 1);
        }
        yy = (yy + 1);
    }
    return fb;
}

fn draw_glyph(fb0: Vec<i32>, text: String, x0: i32, y0: i32, sc: i32, col: i32) -> Vec<i32> {
    let mut fb: Vec<i32> = fb0;
    let mut cx: i32 = x0;
    for ch in text.chars() {
        let mut g: u64 = glyph(ch as i32);
        if (g > 0) {
            for gy in 0..=6 {
                for gx in 0..=4 {
                    let mut bit: u64 = ((g >> ((gy * 5) + gx)) & 1);
                    if (bit > 0) {
                        for py in 0..=(sc - 1) {
                            for px in 0..=(sc - 1) {
                                let mut x: i32 = ((cx + (gx * sc)) + px);
                                let mut y: i32 = ((y0 + (gy * sc)) + py);
                                if ((((x >= 0) && (x < WINW)) && (y >= 0)) && (y < WINH)) {
                                    fb[((y * WINW) + x) as usize] = col;
                                }
                            }
                        }
                    }
                }
            }
        }

        cx = (cx + (6 * sc));
    }
    return fb;
}

fn draw_piece(fb0: Vec<i32>, t: i32, alpha: i32, off_x: i32, off_y: i32) -> Vec<i32> {
    let mut fb: Vec<i32> = fb0;
    if (t < 0) {
        return fb;
    }

    let mut bits: i32 = shape_bits((t * 4));
    for r in 0..=3 {
        for c in 0..=3 {
            if bit_at(bits, r, c) {
                let mut x: i32 = (off_x + (c * PREV));
                let mut y: i32 = (off_y + (r * PREV));
                let mut sa: i32 = 8;
                if (alpha < 1) {
                    sa = 2;
                }

                fb = draw_cell_sprite(fb, x, y, PREV, color_of(t), sa);
            }
        }
    }
    return fb;
}

fn draw_piece_box(
    fb0: Vec<i32>,
    t: i32,
    alpha: i32,
    bx: i32,
    by: i32,
    bw: i32,
    bh: i32,
) -> Vec<i32> {
    let mut fb: Vec<i32> = fb0;
    if (t < 0) {
        return fb;
    }

    let mut bits: i32 = shape_bits((t * 4));
    let mut w: i32 = (piece_w(bits) * PREV);
    let mut h: i32 = (piece_h(bits) * PREV);
    let mut ox: i32 = (bx + ((bw - w) / 2));
    let mut oy: i32 = (by + ((bh - h) / 2));
    for r in 0..=3 {
        for c in 0..=3 {
            if bit_at(bits, r, c) {
                let mut sa: i32 = 8;
                if (alpha < 1) {
                    sa = 2;
                }

                fb = draw_cell_sprite(
                    fb,
                    (ox + (c * PREV)),
                    (oy + (r * PREV)),
                    PREV,
                    color_of(t),
                    sa,
                );
            }
        }
    }
    return fb;
}

fn draw_cell_sprite(fb0: Vec<i32>, x0: i32, y0: i32, sz: i32, col: i32, a: i32) -> Vec<i32> {
    let mut fb: Vec<i32> = fb0;
    if (sz < 6) {
        return fb;
    }

    let mut mm: i32 = 1;
    let mut ix: i32 = (x0 + mm);
    let mut iy: i32 = (y0 + mm);
    let mut iw: i32 = (sz - (2 * mm));
    let mut edge: i32 = ((sz - (2 * mm)) / 5);
    if (edge > 3) {
        edge = 3;
    }

    let mut cr: i32 = ((col >> 16) & 255);
    let mut cg: i32 = ((col >> 8) & 255);
    let mut cb: i32 = (col & 255);
    let mut hr: i32 = (((cr * 55) + (255 * 45)) / 100);
    let mut hg: i32 = (((cg * 55) + (255 * 45)) / 100);
    let mut hb: i32 = (((cb * 55) + (255 * 45)) / 100);
    let mut hcol: i32 = (((hr << 16) | (hg << 8)) | hb);
    let mut sr: i32 = ((cr * 55) / 100);
    let mut sg: i32 = ((cg * 55) / 100);
    let mut sb: i32 = ((cb * 55) / 100);
    let mut scol: i32 = (((sr << 16) | (sg << 8)) | sb);
    let mut gr: i32 = (((cr * 95) + (255 * 5)) / 100);
    let mut gg: i32 = (((cg * 95) + (255 * 5)) / 100);
    let mut gb: i32 = (((cb * 95) + (255 * 5)) / 100);
    let mut gcol: i32 = (((gr << 16) | (gg << 8)) | gb);
    if (a >= 8) {
        fb = fill_rect(fb, ix, iy, iw, iw, col);
        fb = fill_rect(fb, ix, iy, iw, edge, hcol);
        fb = fill_rect(fb, ix, iy, edge, iw, hcol);
        fb = fill_rect(fb, ix, ((iy + iw) - edge), iw, edge, scol);
        fb = fill_rect(fb, ((ix + iw) - edge), iy, edge, iw, scol);
        if (iw > 8) {
            fb = fill_rect(fb, (ix + 4), (iy + 4), (iw - 8), (iw - 8), gcol);
        }
    } else {
        for yy in 0..=(iw - 1) {
            for xx in 0..=(iw - 1) {
                let mut px: i32 = (ix + xx);
                let mut py: i32 = (iy + yy);
                if ((((px >= 0) && (px < WINW)) && (py >= 0)) && (py < WINH)) {
                    let mut idx: i32 = ((py * WINW) + px);
                    let mut br: i32 = ((fb[(idx) as usize] >> 16) & 255);
                    let mut obg: i32 = ((fb[(idx) as usize] >> 8) & 255);
                    let mut bb: i32 = (fb[(idx) as usize] & 255);
                    let mut nr: i32 = (((br * (8 - a)) + (cr * a)) / 8);
                    let mut ng: i32 = (((obg * (8 - a)) + (cg * a)) / 8);
                    let mut nb: i32 = (((bb * (8 - a)) + (cb * a)) / 8);
                    fb[(idx) as usize] = (((nr << 16) | (ng << 8)) | nb);
                }
            }
        }
    }

    return fb;
}

fn make_game(seed: u64, high: i32) -> GameState {
    let mut b = Bag {
        q: vec![],
        nq: vec![],
        p: 0,
        q_peek: 0,
    };
    let mut pair = b.shuffled(seed);
    b = pair.0;
    let mut s = GameState {
        grid: vec![],
        q_hit: false,
        bag: b,
        hold: (-1),
        can_hold: true,
        cur: Piece {
            t: 0,
            rot: 0,
            row: 0,
            col: 0,
        },
        score: 0,
        lines: 0,
        level: 0,
        game_over: false,
        paused: false,
        started: false,
        soft: false,
        accum: 0,
        gms: gravity_ms(0),
        high: high,
        das_l: 0,
        das_r: 0,
        last_t: 0,
        fx_a: 0,
        fx_t: 0,
        fx_cells: vec![],
        seed: pair.1,
        q_ghost_row: 0,
        q_scored: 0,
        q_queue: vec![],
    };
    for i in 0..=((TOTAL * COLS) - 1) {
        s.grid.push(0);
    }
    s = s.spawn_piece((-1));
    return s;
}

fn make_fb() -> Vec<i32> {
    let mut fb: Vec<i32> = vec![];
    for i in 0..=((WINW * WINH) - 1) {
        fb.push(BG);
    }
    return fb;
}

fn render_frame(s0: GameState, fb0: Vec<i32>) -> (GameState, Vec<i32>) {
    let mut s = s0;
    let mut fb: Vec<i32> = fb0;
    for y in 0..=(WINH - 1) {
        let mut t: i32 = ((y * 255) / WINH);
        let mut r: i32 = (((21 * (255 - t)) + (5 * t)) / 255);
        let mut g: i32 = (((8 * (255 - t)) + (1 * t)) / 255);
        let mut bch: i32 = (((61 * (255 - t)) + (10 * t)) / 255);
        fb = fill_rect(fb, 0, y, WINW, 1, (((r << 16) | (g << 8)) | bch));
    }
    fb = fill_rect(fb, (BX - 1), (BY - 1), (BW + 2), (BH + 2), PANELB);
    fb = fill_rect(fb, BX, BY, BW, BH, GRIDBG);
    for gx in 1..=(COLS - 1) {
        fb = fill_rect(fb, (BX + (gx * CELL)), BY, 1, BH, GRIDLN);
    }
    for gy in 1..=(ROWS - 1) {
        fb = fill_rect(fb, BX, (BY + (gy * CELL)), BW, 1, GRIDLN);
    }
    let mut row: i32 = BUF;
    while (row <= (TOTAL - 1)) {
        let mut col: i32 = 0;
        while (col <= (COLS - 1)) {
            let mut v: i32 = s.grid[((row * COLS) + col) as usize];
            if (v > 0) {
                let mut x: i32 = (BX + (col * CELL));
                let mut y: i32 = (BY + ((row - BUF) * CELL));
                fb = draw_cell_sprite(fb, x, y, CELL, color_of((v - 1)), 8);
            }

            col = (col + 1);
        }
        row = (row + 1);
    }
    if (!s.game_over) {
        let mut bits: i32 = shape_bits(((s.cur.t * 4) + s.cur.rot));
        if s.started {
            let mut s2 = s.query_drop();
            s = s2;
            let mut grow: i32 = s.q_ghost_row;
            for r in 0..=3 {
                for c in 0..=3 {
                    if bit_at(bits, r, c) {
                        let mut row: i32 = (grow + r);
                        let mut col: i32 = (s.cur.col + c);
                        if ((row >= BUF) && (row < TOTAL)) {
                            let mut x: i32 = (BX + (col * CELL));
                            let mut y: i32 = (BY + ((row - BUF) * CELL));
                            fb = draw_cell_sprite(fb, x, y, CELL, color_of(s.cur.t), 2);
                        }
                    }
                }
            }
        }

        for r in 0..=3 {
            for c in 0..=3 {
                if bit_at(bits, r, c) {
                    let mut row: i32 = (s.cur.row + r);
                    let mut col: i32 = (s.cur.col + c);
                    if ((row >= BUF) && (row < TOTAL)) {
                        let mut x: i32 = (BX + (col * CELL));
                        let mut y: i32 = (BY + ((row - BUF) * CELL));
                        fb = draw_cell_sprite(fb, x, y, CELL, color_of(s.cur.t), 8);
                    }
                }
            }
        }
    }

    if ((s.fx_a > 0) && ((now_ms() - s.fx_t) < FADE as i64)) {
        let mut age: i64 = (now_ms() - s.fx_t);
        let mut a: i32 = (8 - (age as i32 / 36));
        let mut slot: i32 = 0;
        for i in 0..=(s.fx_a - 1) {
            for c in 0..=(COLS - 1) {
                let mut v: i32 = s.fx_cells[((slot * COLS) + c) as usize];
                if (v > 0) {
                    let mut x: i32 = (BX + (c * CELL));
                    let mut y: i32 = (BY + (slot * CELL));
                    fb = draw_cell_sprite(fb, x, y, CELL, color_of((v - 1)), a);
                }
            }
            slot = (slot + 1);
        }
    }

    let mut lx: i32 = PX0;
    fb = draw_glyph(fb, String::from("SCORE"), lx, 24, 2, WHITE);
    fb = draw_glyph(fb, pad(s.score, 6), lx, 44, 2, CYNE);
    fb = draw_glyph(fb, String::from("HIGH"), lx, 74, 2, WHITE);
    fb = draw_glyph(fb, pad(s.high, 6), lx, 94, 2, YELLOW);
    fb = draw_glyph(fb, String::from("LEVEL"), lx, 124, 2, WHITE);
    fb = draw_glyph(fb, pad(s.level, 2), lx, 144, 2, CYNE);
    fb = draw_glyph(fb, String::from("LINES"), lx, 174, 2, WHITE);
    fb = draw_glyph(fb, pad(s.lines, 3), lx, 194, 2, CYNE);
    fb = draw_glyph(fb, String::from("HOLD"), lx, 232, 2, WHITE);
    fb = fill_rect(fb, (lx - 2), 252, (PW + 4), ((4 * PREV) + 8), PANELB);
    fb = fill_rect(fb, (lx - 1), 253, (PW + 2), ((4 * PREV) + 6), GRIDBG);
    let mut halpha: i32 = 8;
    if (!s.can_hold) {
        halpha = 2;
    }

    fb = draw_piece_box(fb, s.hold, halpha, lx, 256, PW, (4 * PREV));
    let mut rx: i32 = PX1;
    fb = draw_glyph(fb, String::from("NEXT"), rx, 24, 2, WHITE);
    let mut sq = s.query_queue();
    s = sq;
    for i in 0..=4 {
        let mut y: i32 = (44 + (i * ((4 * PREV) + 10)));
        if (i == 0) {
            fb = fill_rect(fb, (rx - 2), (y - 2), (PW + 4), ((4 * PREV) + 8), PANELB);
        }

        fb = fill_rect(fb, (rx - 1), (y - 1), (PW + 2), ((4 * PREV) + 6), GRIDBG);
        let mut pal: i32 = 2;
        if (i == 0) {
            pal = 8;
        }

        fb = draw_piece_box(fb, s.q_queue[(i) as usize], pal, rx, y, PW, (4 * PREV));
    }
    fb = draw_glyph(fb, String::from("CONTROLS"), rx, 502, 2, WHITE);
    fb = draw_glyph(fb, String::from("< > MOVE"), rx, 522, 1, DIM);
    fb = draw_glyph(fb, String::from("V SOFT DROP"), rx, 536, 1, DIM);
    fb = draw_glyph(fb, String::from("^ ROTATE"), rx, 550, 1, DIM);
    fb = draw_glyph(fb, String::from("Z CCW"), rx, 564, 1, DIM);
    fb = draw_glyph(fb, String::from("SPACE HARD DROP"), rx, 578, 1, DIM);
    fb = draw_glyph(fb, String::from("C HOLD"), rx, 592, 1, DIM);
    fb = draw_glyph(fb, String::from("P PAUSE"), rx, 606, 1, DIM);
    fb = draw_glyph(fb, String::from("R RESTART"), rx, 620, 1, DIM);
    fb = draw_glyph(fb, String::from("M SOUND"), rx, 634, 1, DIM);
    fb = draw_glyph(fb, String::from("ESC QUIT"), rx, 648, 1, DIM);
    if ((!s.started) && (!s.game_over)) {
        fb = draw_glyph(
            fb,
            String::from("PRESS ENTER"),
            ((BX + (BW / 2)) - 66),
            ((BY + (BH / 2)) - 7),
            2,
            WHITE,
        );
    }

    if s.paused {
        fb = draw_glyph(
            fb,
            String::from("PAUSED"),
            ((BX + (BW / 2)) - 36),
            ((BY + (BH / 2)) - 7),
            2,
            YELLOW,
        );
    }

    if s.game_over {
        fb = fill_rect(fb, (BX + 10), ((BY + (BH / 2)) - 30), (BW - 20), 64, 0);
        fb = draw_glyph(
            fb,
            String::from("GAME OVER"),
            ((BX + (BW / 2)) - 54),
            ((BY + (BH / 2)) - 21),
            2,
            RED,
        );
        fb = draw_glyph(
            fb,
            String::from("ENTER TO RESTART"),
            ((BX + (BW / 2)) - 48),
            ((BY + (BH / 2)) + 8),
            1,
            DIM,
        );
    }

    for sy in 0..=((BH / 2) - 1) {
        for sx in 0..=(BW - 1) {
            let mut x: i32 = (BX + sx);
            let mut y: i32 = ((BY + (sy * 2)) + 1);
            let mut v: i32 = fb[((y * WINW) + x) as usize];
            let mut r: i32 = ((v >> 16) & 255);
            let mut g: i32 = ((v >> 8) & 255);
            let mut b: i32 = (v & 255);
            r = ((r * 90) / 100);
            g = ((g * 90) / 100);
            b = ((b * 90) / 100);
            fb[((y * WINW) + x) as usize] = (((r << 16) | (g << 8)) | b);
        }
    }
    return (s, fb);
}

fn run_gui(s0: GameState) {
    let mut s = s0;
    audio_init();
    if (!win_open(WINW, WINH, String::from("TETRIS"))) {
        eprintln!("[ERROR] {}", String::from("could not open window"));
    }

    let mut prev: Vec<i32> = vec![];
    s.last_t = now_ms();
    loop {
        if (!win_alive()) {
            break;
        }

        let mut now: i32 = now_ms() as i32;
        let mut dt: i64 = (now as i64 - s.last_t);
        if (dt > 250 as i64) {
            dt = 250;
        }

        s.last_t = now as i64;
        let mut keys: Vec<i32> = win_keys();
        let mut quit: bool = false;
        for k in keys.iter().cloned() {
            let mut fresh: bool = true;
            for p in prev.iter().cloned() {
                if (p == k) {
                    fresh = false;
                }
            }
            if fresh {
                if (k == 27) {
                    quit = true;
                }

                if (k == 13) {
                    if s.game_over {
                        s = s.restart();
                    } else {
                        if (!s.started) {
                            s.started = true;
                        }
                    }
                }

                if (k == 37) {
                    if ((s.started && (!s.paused)) && (!s.game_over)) {
                        s = s.move_h((-1));
                    }

                    s.das_l = now as i64;
                }

                if (k == 39) {
                    if ((s.started && (!s.paused)) && (!s.game_over)) {
                        s = s.move_h(1);
                    }

                    s.das_r = now as i64;
                }

                if ((k == 38) || (k == 88)) {
                    if ((s.started && (!s.paused)) && (!s.game_over)) {
                        s = s.rotate(1);
                    }
                }

                if (k == 90) {
                    if ((s.started && (!s.paused)) && (!s.game_over)) {
                        s = s.rotate((-1));
                    }
                }

                if (k == 32) {
                    if ((s.started && (!s.paused)) && (!s.game_over)) {
                        s = s.hard_drop();
                    }
                }

                if (k == 67) {
                    if ((s.started && (!s.paused)) && (!s.game_over)) {
                        s = s.do_hold();
                    }
                }

                if (k == 80) {
                    s = s.toggle_pause();
                }

                if (k == 82) {
                    s = s.restart();
                }

                if (k == 77) {
                    s = s.toggle_mute();
                }
            }
        }
        if quit {
            break;
        }

        let mut down_held: bool = false;
        for k in keys.iter().cloned() {
            if ((k == 40) || (k == 86)) {
                down_held = true;
            }
        }
        s.soft = down_held;
        for k in keys.iter().cloned() {
            if (k == 37) {
                if ((now as i64 - s.das_l) > DAS_DELAY as i64) {
                    if ((s.started && (!s.paused)) && (!s.game_over)) {
                        s = s.move_h((-1));
                    }

                    s.das_l = (now as i64 - DAS_RATE as i64);
                }
            }

            if (k == 39) {
                if ((now as i64 - s.das_r) > DAS_DELAY as i64) {
                    if ((s.started && (!s.paused)) && (!s.game_over)) {
                        s = s.move_h(1);
                    }

                    s.das_r = (now as i64 - DAS_RATE as i64);
                }
            }
        }
        prev = keys;
        s = s.gravity(dt);
        let mut fb = make_fb();
        let mut rf = render_frame(s, fb);
        s = rf.0;
        if (!win_present(rf.1, WINW, WINH)) {
            break;
        }
    }
    win_close();
}

fn run_selftest(s0: GameState) {
    let mut s = s0;
    println!("{}", String::from("POLY TETRIS self-test"));
    let mut fails: i32 = 0;
    for t in 0..=6 {
        for rot in 0..=3 {
            let mut bits: i32 = shape_bits(((t * 4) + rot));
            let mut n: i32 = 0;
            for r in 0..=3 {
                for c in 0..=3 {
                    if bit_at(bits, r, c) {
                        n = (n + 1);
                    }
                }
            }
            if (n != 4) {
                fails = (fails + 1);
                println!("{}", String::from("FAIL shape cells"));
            }
        }
    }
    for rot in 0..=3 {
        if (shape_bits((4 + rot)) != 51) {
            fails = (fails + 1);
            println!("{}", String::from("FAIL O invariance"));
        }
    }
    if (gravity_ms(0) != 800 as i64) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL gravity0"));
    }

    if (gravity_ms(29) != 16 as i64) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL gravity29"));
    }

    if (gravity_ms(99) != 16 as i64) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL gravity-cap"));
    }

    if (((glyph(48) < 1) || (glyph(90) < 1)) || (glyph(64) > 0)) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL font"));
    }

    if (color_of(0) != 0x2BF2FF) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL color"));
    }

    if (pad(42, 6) != String::from("000042")) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL pad"));
    }

    let mut sg = make_game(s.seed, highscore_load());
    let mut g = sg;
    if (g.cur.row != BUF) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL spawn row"));
    }

    if (g.cur.col != ((COLS - piece_w(shape_bits((g.cur.t * 4)))) / 2)) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL spawn col"));
    }

    let mut counts: Vec<i32> = vec![0, 0, 0, 0, 0, 0, 0];
    for i in 0..=6 {
        counts[(g.bag.q[(i) as usize]) as usize] = (counts[(g.bag.q[(i) as usize]) as usize] + 1);
    }
    for t in 0..=6 {
        if (counts[(t) as usize] != 1) {
            fails = (fails + 1);
            println!("{}", String::from("FAIL bag"));
        }
    }
    let mut okq: bool = true;
    for i in 0..=4 {
        let mut bp = g.bag.peek(i);
        g.bag = bp;
        let mut expect: i32 = g.bag.q[(1 + i) as usize];
        if (g.bag.q_peek != expect) {
            okq = false;
        }
    }
    if (!okq) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL queue-peek"));
    }

    let mut gt: i32 = g.cur.t;
    let mut grot: i32 = g.cur.rot;
    let mut grow: i32 = g.cur.row;
    let mut gcol: i32 = g.cur.col;
    g = g.collides(gt, grot, grow, gcol);
    if g.q_hit {
        fails = (fails + 1);
        println!("{}", String::from("FAIL collide-empty"));
    }

    g = g.collides(gt, grot, grow, (-1));
    if (!g.q_hit) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL collide-wall"));
    }

    g = g.collides(gt, grot, TOTAL, gcol);
    if (!g.q_hit) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL collide-floor"));
    }

    g.grid[((10 * COLS) + 5) as usize] = 4;
    g = g.query_cell(10, 5);
    if (g.q_scored != 4) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL cell-read"));
    }

    g = g.query_cell(11, 5);
    if (g.q_scored != 0) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL cell-empty"));
    }

    g = g.query_cell((-1), 0);
    if (g.q_scored != 0) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL cell-bounds"));
    }

    let mut sg2 = make_game(s.seed, highscore_load());
    let mut g2 = sg2;
    for c in 0..=(COLS - 1) {
        g2.grid[(((TOTAL - 1) * COLS) + c) as usize] = 3;
    }
    let mut before: i32 = g2.score;
    g2 = g2.clear_lines();
    if (g2.score != (before + 40)) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL clear-score"));
    }

    if (g2.lines != 1) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL clear-lines"));
    }

    g2 = g2.query_cell((TOTAL - 1), 5);
    if (g2.q_scored != 0) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL clear-removes"));
    }

    let mut sg9 = make_game(s.seed, highscore_load());
    let mut g9 = sg9;
    let mut c: i32 = 0;
    while (c <= (COLS - 1)) {
        g9.grid[(((TOTAL - 1) * COLS) + c) as usize] = 1;
        c = (c + 1);
    }
    g9.grid[(((TOTAL - 2) * COLS) + 0) as usize] = 2;
    g9.grid[(((TOTAL - 2) * COLS) + 1) as usize] = 2;
    g9 = g9.clear_lines();
    if (g9.grid[(((TOTAL - 1) * COLS) + 0) as usize] != 2) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL stack-survives-a"));
    }

    if (g9.grid[(((TOTAL - 1) * COLS) + 1) as usize] != 2) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL stack-survives-b"));
    }

    if (g9.grid[(((TOTAL - 1) * COLS) + 5) as usize] != 0) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL stack-cleared-col"));
    }

    if (g9.grid[(((TOTAL - 2) * COLS) + 0) as usize] != 0) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL stack-row-above-empty"));
    }

    let mut sg3 = make_game(s.seed, highscore_load());
    let mut g3 = sg3;
    let mut r: i32 = (TOTAL - 4);
    while (r <= (TOTAL - 1)) {
        let mut c: i32 = 0;
        while (c <= (COLS - 1)) {
            g3.grid[((r * COLS) + c) as usize] = 3;
            c = (c + 1);
        }
        r = (r + 1);
    }
    let mut b3: i32 = g3.score;
    g3 = g3.clear_lines();
    if (g3.score != (b3 + 800)) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL tetris-score"));
    }

    let mut sg4 = make_game(s.seed, highscore_load());
    let mut g4 = sg4;
    g4.started = true;
    g4 = g4.hard_drop();
    let mut landed: i32 = 0;
    let mut r: i32 = BUF;
    while (r <= (TOTAL - 1)) {
        let mut c: i32 = 0;
        while (c <= (COLS - 1)) {
            if (g4.grid[((r * COLS) + c) as usize] > 0) {
                landed = (landed + 1);
            }

            c = (c + 1);
        }
        r = (r + 1);
    }
    if (landed != 4) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL harddrop-lock"));
    }

    let mut sg5 = make_game(s.seed, highscore_load());
    let mut g5 = sg5;
    g5.started = true;
    let mut t0: i32 = g5.cur.t;
    g5 = g5.do_hold();
    if (g5.hold != t0) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL hold-store"));
    }

    let mut t1: i32 = g5.cur.t;
    g5 = g5.do_hold();
    if ((g5.hold != t0) || (g5.cur.t != t1)) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL hold-deny"));
    }

    let mut sg6 = make_game(s.seed, highscore_load());
    let mut g6 = sg6;
    g6 = g6.query_drop();
    let mut exp: i32 = (TOTAL - piece_h(shape_bits((g6.cur.t * 4))));
    if (g6.q_ghost_row != exp) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL ghost"));
    }

    let mut sg7 = make_game(s.seed, highscore_load());
    let mut g7 = sg7;
    g7.started = true;
    for i in 0..=20 {
        g7 = g7.move_h((-1));
    }
    let mut minc: i32 = 99;
    let mut bits7: i32 = shape_bits(((g7.cur.t * 4) + g7.cur.rot));
    for r in 0..=3 {
        for c in 0..=3 {
            if bit_at(bits7, r, c) {
                if ((g7.cur.col + c) < minc) {
                    minc = (g7.cur.col + c);
                }
            }
        }
    }
    if (minc < 0) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL wall-clamp"));
    }

    let mut sg8 = make_game(s.seed, highscore_load());
    let mut g8 = sg8;
    g8 = g8.query_queue();
    if ((g8.q_queue.len() as i32) != 5) {
        fails = (fails + 1);
        println!("{}", String::from("FAIL queue-len"));
    }

    for i in 0..=4 {
        if ((g8.q_queue[(i) as usize] < 0) || (g8.q_queue[(i) as usize] > 6)) {
            fails = (fails + 1);
            println!("{}", String::from("FAIL queue-range"));
        }
    }
    if (fails == 0) {
        println!("{}", String::from("ALL TESTS PASSED"));
    } else {
        println!(
            "{}",
            format!("{}{}", format!("{:?}", fails), String::from(" FAILURES"))
        );
    }
}

fn main() {
    let mut args = get_args();
    let mut test_mode: bool = false;
    for a in args.iter().cloned() {
        if (a == String::from("--test")) {
            test_mode = true;
        }
    }
    let mut s = make_game(rng_seed(), highscore_load());
    if test_mode {
        run_selftest(s)
    } else {
        run_gui(s)
    }
}

// ================= platform layer (Rust) =================

use minifb::{Key, Window, WindowOptions};
use std::cell::RefCell;

thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
    static AUDIO_TX: RefCell<Option<std::sync::mpsc::Sender<Vec<i16>>>> = const { RefCell::new(None) };
    static PHASE: RefCell<f32> = const { RefCell::new(0.0) };
}

// ---- window ----

fn win_open(width: i32, height: i32, title: String) -> bool {
    let mut opts = WindowOptions::default();
    opts.resize = false;
    opts.scale = minifb::Scale::X1;
    match Window::new(&title, width as usize, height as usize, opts) {
        Ok(mut win) => {
            win.limit_update_rate(Some(std::time::Duration::from_micros(16666)));
            WINDOW.with(|c| *c.borrow_mut() = Some(win));
            true
        }
        Err(_) => false,
    }
}

fn win_alive() -> bool {
    WINDOW.with(|c| match c.borrow().as_ref() {
        Some(w) => w.is_open() && !w.is_key_down(Key::Escape),
        None => false,
    })
}

// Poly key codes: Enter=13 Left=37 Up=38 Right=39 Down=40 Space=32 Z=90 X=88 C=67 P=80 R=82 M=77
fn map_key(k: Key) -> Option<i32> {
    Some(match k {
        Key::Enter => 13,
        Key::Left => 37,
        Key::Up => 38,
        Key::Right => 39,
        Key::Down => 40,
        Key::Space => 32,
        Key::Z => 90,
        Key::X => 88,
        Key::C => 67,
        Key::P => 80,
        Key::R => 82,
        Key::M => 77,
        _ => return None,
    })
}

fn win_keys() -> Vec<i32> {
    WINDOW.with(|c| match c.borrow().as_ref() {
        Some(w) => w.get_keys().iter().filter_map(|k| map_key(*k)).collect(),
        None => Vec::new(),
    })
}

fn win_present(pixels: Vec<i32>, width: i32, height: i32) -> bool {
    let frame: Vec<u32> = pixels.iter().map(|p| *p as u32).collect();
    WINDOW.with(|c| match c.borrow_mut().as_mut() {
        Some(w) => w
            .update_with_buffer(&frame, width as usize, height as usize)
            .is_ok(),
        None => false,
    })
}

fn win_close() {
    WINDOW.with(|c| *c.borrow_mut() = None);
}

// ---- misc platform ----

fn get_args() -> Vec<String> {
    std::env::args().collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn highscore_load() -> i32 {
    std::fs::read_to_string("tetris.hs")
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(0)
}

fn highscore_save(v: i32) {
    let _ = std::fs::write("tetris.hs", v.to_string());
}

fn rng_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ (d.as_secs() << 32))
        .unwrap_or(42);
    nanos ^ (nanos << 31) ^ 0x9E3779B97F4A7C15
}

// ---- audio: tiny synth via ALSA, no asset dependencies ----
// Poly kind codes:
//   0 move, 1 rotate, 2 lock, 3 clear, 4 tetris, 6 hard drop,
//   7 game over, 8 mute blip

const RATE: usize = 44100;

fn render_tone(freq: u32, dur_ms: u64, wave: u8, vol: f32, slide: u32) -> Vec<i16> {
    let n = (RATE * dur_ms as usize) / 1000;
    let f0 = freq as f32;
    let f1 = if slide > 0 { slide as f32 } else { f0 };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / n.max(1) as f32;
        let f = f0 + (f1 - f0) * t;
        PHASE.with(|p| {
            let mut ph = p.borrow_mut();
            *ph += f / RATE as f32;
            if *ph >= 1.0 {
                *ph -= 1.0;
            }
            let s = match wave {
                0 => {
                    if *ph < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                1 => 4.0 * (*ph - 0.5).abs() - 1.0,
                _ => 2.0 * *ph - 1.0,
            };
            let env = (1.0 - t) * (1.0 - t);
            out.push((s * vol * env * 32767.0) as i16);
        });
    }
    out
}

fn mix(a: &[i16], b: &[i16]) -> Vec<i16> {
    let n = a.len().max(b.len());
    (0..n)
        .map(|i| {
            let x = a.get(i).copied().unwrap_or(0) as i32;
            let y = b.get(i).copied().unwrap_or(0) as i32;
            (x + y).clamp(i16::MIN as i32, i16::MAX as i32) as i16
        })
        .collect()
}

fn audio_init() {
    AUDIO_TX.with(|c| {
        if c.borrow().is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<Vec<i16>>();
        *c.borrow_mut() = Some(tx);
        std::thread::spawn(move || {
            let pcm = match alsa::pcm::PCM::new("default", alsa::Direction::Playback, false) {
                Ok(p) => p,
                Err(_) => return,
            };
            {
                let hwp = match alsa::pcm::HwParams::any(&pcm) {
                    Ok(h) => h,
                    Err(_) => return,
                };
                let _ = hwp.set_channels(1);
                let _ = hwp.set_format(alsa::pcm::Format::s16());
                let _ = hwp.set_rate(RATE as u32, alsa::ValueOr::Nearest);
                let _ = hwp.set_access(alsa::pcm::Access::RWInterleaved);
                if pcm.hw_params(&hwp).is_err() {
                    return;
                }
                let _ = pcm.prepare();
            }
            let io = match pcm.io_i16() {
                Ok(io) => io,
                Err(_) => return,
            };
            while let Ok(samples) = rx.recv() {
                let mut i = 0;
                while i < samples.len() {
                    match io.writei(&samples[i..]) {
                        Ok(n) if n > 0 => i += n,
                        _ => {
                            let _ = pcm.prepare();
                        }
                    }
                }
            }
        });
    });
}

fn audio_play(kind: i32) {
    AUDIO_TX.with(|c| {
        if let Some(tx) = c.borrow().as_ref() {
            let msg: Option<Vec<i16>> = match kind {
                0 => Some(render_tone(180, 40, 0, 0.04, 0)),
                1 => Some(render_tone(280, 50, 0, 0.05, 0)),
                2 => Some(render_tone(120, 70, 0, 0.07, 80)),
                3 => Some(mix(
                    &render_tone(440, 80, 1, 0.08, 0),
                    &render_tone(660, 100, 1, 0.08, 0),
                )),
                4 => {
                    let mut v = mix(
                        &render_tone(523, 100, 1, 0.10, 0),
                        &render_tone(659, 100, 1, 0.10, 0),
                    );
                    v = mix(&v, &render_tone(784, 100, 1, 0.10, 0));
                    v = mix(&v, &render_tone(1046, 180, 1, 0.10, 0));
                    Some(v)
                }
                6 => Some(render_tone(90, 50, 2, 0.05, 50)),
                7 => {
                    let mut v = render_tone(220, 180, 0, 0.08, 110);
                    v = mix(&v, &render_tone(160, 180, 0, 0.08, 80));
                    Some(v)
                }
                8 => Some(render_tone(300, 30, 0, 0.03, 0)),
                _ => None,
            };
            if let Some(v) = msg {
                let _ = tx.send(v);
            }
        }
    });
}
