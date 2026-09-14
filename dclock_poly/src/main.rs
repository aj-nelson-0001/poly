// Generated from Poly source code
#![allow(
    unused_variables,
    unused_mut,
    unused_imports,
    dead_code,
    unused_parens,
    unreachable_patterns
)]

const DIGIT_WIDTH: i32 = 5;
const DIGIT_HEIGHT: i32 = 5;
const INITIAL_WIDTH: i32 = 450;
const INITIAL_HEIGHT: i32 = 100;
const BACKGROUND: i32 = 0x000000;
const FOREGROUND: i32 = 0xFFFFFF;
const COLON_COLOR: i32 = 0xFFFF00;

struct DigitBuffer {
    buffer: Vec<i32>,
    width: i32,
    height: i32,
}

fn digit_bits(digit: i32) -> i32 {
    match digit {
        0 => {
            return 0b1111110001100011000111111;
        }
        1 => {
            return 0b0010001100001000010001110;
        }
        2 => {
            return 0b1111100001111111000011111;
        }
        3 => {
            return 0b1111100001111110000111111;
        }
        4 => {
            return 0b1000110001111110000100001;
        }
        5 => {
            return 0b1111110000111110000111111;
        }
        6 => {
            return 0b1111110000111111000111111;
        }
        7 => {
            return 0b1111100001000100010001000;
        }
        8 => {
            return 0b1111110001111111000111111;
        }
        _ => {
            return 0b1111110001111110000111111;
        }
    }
}

fn make_digit(digit: i32, scale_x: i32, scale_y: i32) -> DigitBuffer {
    let mut width: i32 = (DIGIT_WIDTH * scale_x);
    let mut height: i32 = (DIGIT_HEIGHT * scale_y);
    let mut total: i32 = (width * height);
    let mut buffer: Vec<i32> = vec![];
    for i in 0..=(total - 1) {
        buffer.push(BACKGROUND);
    }
    let mut bits: i32 = digit_bits(digit);
    for y in 0..=(DIGIT_HEIGHT - 1) {
        for x in 0..=(DIGIT_WIDTH - 1) {
            let mut bit_index: i32 = (((DIGIT_WIDTH * DIGIT_HEIGHT) - 1) - ((y * DIGIT_WIDTH) + x));
            if (((bits >> bit_index) & 1) == 1) {
                for dy in 0..=(scale_y - 1) {
                    for dx in 0..=(scale_x - 1) {
                        let mut index: i32 =
                            ((((y * scale_y) + dy) * width) + ((x * scale_x) + dx));
                        buffer[(index) as usize] = FOREGROUND;
                    }
                }
            }
        }
    }
    return DigitBuffer {
        buffer: buffer,
        width: width,
        height: height,
    };
}

fn create_digit_buffers(scale_x: i32, scale_y: i32) -> Vec<DigitBuffer> {
    let mut buffers: Vec<DigitBuffer> = vec![];
    for d in 0..=9 {
        buffers.push(make_digit(d, scale_x, scale_y));
    }
    return buffers;
}

fn draw_colon(buffer: Vec<i32>, width: i32, height: i32, x: i32, colon_width: i32) -> Vec<i32> {
    let mut out: Vec<i32> = buffer;
    let mut colon_height: i32 = (height / 6);
    let mut gap: i32 = (height / 20);
    let mut total_colon_height: i32 = ((2 * colon_height) + gap);
    let mut start_y: i32 = ((height - total_colon_height) / 2);
    let mut bottom_y: i32 = ((start_y + colon_height) + gap);
    for dy in 0..=(colon_height - 1) {
        for dx in 0..=(colon_width - 1) {
            if (((start_y + dy) < height) && ((x + dx) < width)) {
                let mut index: i32 = (((start_y + dy) * width) + (x + dx));
                out[(index) as usize] = COLON_COLOR;
            }

            if (((bottom_y + dy) < height) && ((x + dx) < width)) {
                let mut index: i32 = (((bottom_y + dy) * width) + (x + dx));
                out[(index) as usize] = COLON_COLOR;
            }
        }
    }
    return out;
}

fn expand_time(hms: Vec<i32>) -> Vec<i32> {
    let mut digits: Vec<i32> = vec![];
    for i in 0..=2 {
        let mut value: i32 = hms[(i) as usize];
        digits.push((value / 10));
        digits.push((value % 10));
    }
    return digits;
}

fn draw_time(
    digit_buffers: Vec<DigitBuffer>,
    buffer: Vec<i32>,
    time: Vec<i32>,
    width: i32,
    height: i32,
) -> Vec<i32> {
    let mut out: Vec<i32> = buffer;
    let mut digit_width: i32 = digit_buffers[(0) as usize].width;
    let mut digit_height: i32 = digit_buffers[(0) as usize].height;
    let mut spacing: i32 = (digit_width / 4);
    let mut colon_width: i32 = (digit_width / 4);
    let mut total_width: i32 = (((6 * digit_width) + (2 * colon_width)) + (5 * spacing));
    let mut start_x: i32 = 0;
    if (width > total_width) {
        start_x = ((width - total_width) / 2);
    }

    let mut start_y: i32 = ((height - digit_height) / 2);
    let mut x: i32 = start_x;
    for i in 0..=5 {
        let mut digit: i32 = time[(i) as usize];
        if ((x + digit_width) <= width) {
            let mut src_width: i32 = digit_buffers[(digit) as usize].width;
            let mut src_height: i32 = digit_buffers[(digit) as usize].height;
            let mut copy_width: i32 = digit_width;
            if ((width - x) < copy_width) {
                copy_width = (width - x);
            }

            for y in 0..=(src_height - 1) {
                let mut target_y: i32 = (start_y + y);
                if ((target_y >= 0) && (target_y < height)) {
                    for dx in 0..=(copy_width - 1) {
                        let mut src_index: i32 = ((y * src_width) + dx);
                        let mut dst_index: i32 = (((target_y * width) + x) + dx);
                        out[(dst_index) as usize] =
                            digit_buffers[(digit) as usize].buffer[(src_index) as usize];
                    }
                }
            }
        }

        x = ((x + digit_width) + spacing);
        if ((i == 1) || (i == 3)) {
            if ((x + colon_width) <= width) {
                out = draw_colon(out, width, height, x, colon_width);
            }

            x = ((x + colon_width) + spacing);
        }
    }
    return out;
}

fn render_clock(time: Vec<i32>, width: i32, height: i32) -> Vec<i32> {
    let mut buffer: Vec<i32> = vec![];
    for i in 0..=(width * height) {
        buffer.push(BACKGROUND);
    }
    let mut scale_x: i32 = (width / 50);
    if (scale_x < 1) {
        scale_x = 1;
    }

    let mut scale_y: i32 = (height / 5);
    if (scale_y < 1) {
        scale_y = 1;
    }

    let mut digit_buffers: Vec<DigitBuffer> = create_digit_buffers(scale_x, scale_y);
    return draw_time(digit_buffers, buffer, expand_time(time), width, height);
}

fn main() {
    let mut width: i32 = INITIAL_WIDTH;
    let mut height: i32 = INITIAL_HEIGHT;
    if (!window_open(String::from("Digital Clock"), width, height)) {
        eprintln!("[ERROR] {}", String::from("could not open window"));
    }

    let mut buffer: Vec<i32> = vec![];
    loop {
        if window_should_close() {
            break;
        }

        let mut size: Vec<i32> = window_resize();
        if (size[(0) as usize] > 0) {
            width = size[(0) as usize];
            height = size[(1) as usize];
        }

        let mut time: Vec<i32> = clock_seconds_now();
        buffer = render_clock(time, width, height);
        if (!window_present(buffer, width, height)) {
            break;
        }
    }
}

use chrono::Timelike;
use std::cell::RefCell;
use std::time::{Duration, Instant};

// -- Window state (minifb) --
// Single-threaded GUI loop, so thread-local cells keep this warning-free
// (no `static mut` references).
thread_local! {
    static WINDOW: RefCell<Option<minifb::Window>> = const { RefCell::new(None) };
    static LAST_SIZE: RefCell<Option<(i32, i32)>> = const { RefCell::new(None) };
}

// -- Clock state (chrono) --
thread_local! {
    static CLOCK_LAST_HMS: RefCell<[i32; 3]> = const { RefCell::new([-1, -1, -1]) };
    static CLOCK_LAST_TICK: RefCell<Option<Instant>> = const { RefCell::new(None) };
}

fn window_open(title: String, width: i32, height: i32) -> bool {
    let window = minifb::Window::new(
        &title,
        width as usize,
        height as usize,
        minifb::WindowOptions {
            resize: true,
            ..minifb::WindowOptions::default()
        },
    );
    match window {
        Ok(w) => {
            WINDOW.with(|cell| *cell.borrow_mut() = Some(w));
            true
        }
        Err(_) => false,
    }
}

fn window_should_close() -> bool {
    WINDOW.with(|cell| match cell.borrow().as_ref() {
        Some(w) => !w.is_open() || w.is_key_down(minifb::Key::Escape),
        None => true,
    })
}

// Returns the current size as [width, height]; [-1, -1] when unchanged.
fn window_resize() -> Vec<i32> {
    WINDOW.with(|cell| match cell.borrow().as_ref() {
        Some(w) => {
            let (w, h) = w.get_size();
            let (pw, ph) = (w as i32, h as i32);
            let unchanged = LAST_SIZE.with(|c| match *c.borrow() {
                Some((lw, lh)) => lw == pw && lh == ph,
                None => false,
            });
            if unchanged {
                [-1, -1].to_vec()
            } else {
                LAST_SIZE.with(|c| *c.borrow_mut() = Some((pw, ph)));
                [pw, ph].to_vec()
            }
        }
        None => [-1, -1].to_vec(),
    })
}

fn window_present(pixels: Vec<i32>, width: i32, height: i32) -> bool {
    let frame: Vec<u32> = pixels.iter().map(|p| *p as u32).collect();
    WINDOW.with(|cell| match cell.borrow_mut().as_mut() {
        Some(w) => w
            .update_with_buffer(&frame, width as usize, height as usize)
            .is_ok(),
        None => false,
    })
}

// Returns the current local time as [hour, minute, second],
// refreshed at most once per second.
fn clock_seconds_now() -> Vec<i32> {
    let fresh = CLOCK_LAST_TICK.with(|c| match *c.borrow() {
        Some(t) => t.elapsed() >= Duration::from_secs(1),
        None => true,
    });
    if !fresh {
        return CLOCK_LAST_HMS.with(|c| c.borrow().to_vec());
    }
    let now = chrono::Local::now();
    let hms = [now.hour() as i32, now.minute() as i32, now.second() as i32];
    CLOCK_LAST_HMS.with(|c| *c.borrow_mut() = hms);
    CLOCK_LAST_TICK.with(|c| *c.borrow_mut() = Some(Instant::now()));
    hms.to_vec()
}
