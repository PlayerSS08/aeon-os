//! GOP framebuffer text console: bitmap font, scrolling log area, splash logo and a status bar.
//!
//! The console never reads the framebuffer (it may be uncached or write-combined, where reads are
//! very slow). Text lives in a RAM cell grid and scrolling redraws from it.

use crate::bootinfo::FbInfo;
use crate::font;
use crate::global::Global;
use crate::logfmt::Level;
use crate::pixel::PixelFormat;

use crate::splash::{self, MAX_SCALE};
const MAX_COLS: usize = 256;
const MAX_ROWS: usize = 128;

#[derive(Clone, Copy)]
struct Cell {
    ch: u8,
    color: u8,
}

// All-zero so the grid lives in .bss instead of bloating the image.
const BLANK: Cell = Cell { ch: 0, color: 0 };

static CELLS: Global<[Cell; MAX_COLS * MAX_ROWS]> = Global::new([BLANK; MAX_COLS * MAX_ROWS]);

pub const COLOR_ERROR: u8 = 0;
pub const COLOR_WARN: u8 = 1;
pub const COLOR_INFO: u8 = 2;
pub const COLOR_DEBUG: u8 = 3;
pub const COLOR_TRACE: u8 = 4;

const PALETTE: [(u8, u8, u8); 5] = [
    (255, 96, 96),   // error
    (240, 200, 80),  // warn
    (220, 220, 220), // info
    (150, 170, 190), // debug
    (100, 110, 120), // trace
];
const BG: (u8, u8, u8) = (0, 0, 0);
const BAR_BG: (u8, u8, u8) = (40, 60, 90);
const BAR_FG: (u8, u8, u8) = (235, 240, 250);
const LOGO_RGB: (u8, u8, u8) = (255, 255, 255);
const TRACK_RGB: (u8, u8, u8) = (36, 40, 50);
const PANIC_BG: (u8, u8, u8) = (28, 8, 10);
const PANIC_LOGO: (u8, u8, u8) = (255, 110, 110);


pub const BANNER_COLS: usize = 37;
pub const BANNER_ROWS: usize = 6;
pub const BANNER: [[u8; BANNER_COLS]; BANNER_ROWS] = [
    [b' ', b' ', 0x80, 0x80, 0x80, b' ', b' ',   b' ', b' ', b' ',   0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,   b' ', b' ', b' ',   b' ', 0x80, 0x80, 0x80, 0x80, 0x80, b' ',   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80],
    [b' ', 0x80, 0x80, b' ', 0x80, 0x80, b' ',   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', b' ', b' ',   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80,   b' ', b' ', b' ',   0x80, 0x80, 0x80, b' ', b' ', 0x80, 0x80],
    [0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80,   b' ', b' ', b' ',   0x80, 0x80, 0x80, 0x80, 0x80, b' ', b' ',   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80,   b' ', b' ', b' ',   0x80, 0x80, b' ', 0x80, b' ', 0x80, 0x80],
    [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', b' ', b' ',   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80,   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', 0x80, 0x80, 0x80],
    [0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80,   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', b' ', b' ',   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80,   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80],
    [0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80,   b' ', b' ', b' ',   0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,   b' ', b' ', b' ',   b' ', 0x80, 0x80, 0x80, 0x80, 0x80, b' ',   b' ', b' ', b' ',   0x80, 0x80, b' ', b' ', b' ', 0x80, 0x80],
];

pub fn color_for(level: Level) -> u8 {
    match level {
        Level::Error => COLOR_ERROR,
        Level::Warn => COLOR_WARN,
        Level::Info => COLOR_INFO,
        Level::Debug => COLOR_DEBUG,
        Level::Trace => COLOR_TRACE,
    }
}

pub struct Console {
    base: *mut u32,
    width: usize,
    height: usize,
    stride: usize,
    format: PixelFormat,
    scale: usize,
    cols: usize,
    rows: usize,
    first_row: usize,
    log_rows: usize,
    cur_col: usize,
    cur_row: usize,
    /// While true the log is not painted: the screen shows only the logo and the progress bar.
    splash: bool,
    /// Last bar position drawn, in thousandths.
    bar_pm: u32,
    logo_row: usize,
}

impl Console {
    /// Creates a console for the framebuffer described by `info`.
    ///
    /// # Safety
    /// `info.base`/`info.size` must describe a mapped, writable 32-bit-per-pixel framebuffer that
    /// stays valid (and is not otherwise written) for as long as the console is used.
    pub unsafe fn new(info: &FbInfo) -> Option<Console> {
        if info.width == 0 || info.height == 0 || info.stride < info.width {
            return None;
        }
        // The framebuffer must be large enough for the geometry we will draw into.
        let needed = info.stride.checked_mul(info.height)?.checked_mul(4)?;
        if info.size < needed {
            return None;
        }
        let scale = splash::scale_for_width(info.width);
        let cols = (info.width / (8 * scale)).min(MAX_COLS);
        let rows = (info.height / (16 * scale)).min(MAX_ROWS);
        if cols < 20 || rows < 6 {
            return None;
        }
        let mut c = Console {
            base: info.base as *mut u32,
            width: info.width,
            height: info.height,
            stride: info.stride,
            format: info.format,
            scale,
            cols,
            rows,
            first_row: 0,
            log_rows: rows - 1,
            cur_col: 0,
            cur_row: 0,
            splash: false,
            bar_pm: 0,
            logo_row: 0,
        };
        c.logo_row = rows.saturating_sub(BANNER_ROWS) / 3;
        // Put the log below the logo when the screen is big enough to keep a useful log area.
        let _ = info.width;
        let first = (rows.saturating_sub(BANNER_ROWS)) / 3 + BANNER_ROWS + 2;
        if rows >= first + 4 {
            c.first_row = first;
            c.log_rows = rows - 1 - first;
        }
        Some(c)
    }

    fn cell_w(&self) -> usize {
        8 * self.scale
    }

    fn cell_h(&self) -> usize {
        16 * self.scale
    }

    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    fn px(&self, c: (u8, u8, u8)) -> u32 {
        self.format.pack(c.0, c.1, c.2)
    }

    /// The pixels `x..x + len` of scan line `y`, clipped to the visible area. The framebuffer is
    /// device memory owned by the console, hence `&self` yielding a mutable span.
    #[allow(clippy::mut_from_ref)]
    fn row_mut(&self, x: usize, y: usize, len: usize) -> Option<&mut [u32]> {
        if y >= self.height || x >= self.width {
            return None;
        }
        let len = len.min(self.width - x);
        // SAFETY: `x + len <= width <= stride` and `y < height`, and `new` checked that the
        // framebuffer holds `stride * height` pixels, so the span is inside it. The console is the
        // only writer and callers never hold two rows at once.
        Some(unsafe { core::slice::from_raw_parts_mut(self.base.add(y * self.stride + x), len) })
    }

    fn fill_rect(&self, x: usize, y: usize, w: usize, h: usize, v: u32) {
        for yy in y..(y + h).min(self.height) {
            if let Some(row) = self.row_mut(x, yy, w) {
                row.fill(v);
            }
        }
    }

    pub fn clear(&mut self) {
        let bg = self.px(BG);
        self.fill_rect(0, 0, self.width, self.height, bg);
        // SAFETY: single-threaded; no other reference to the cell grid is live here.
        let cells = unsafe { CELLS.get() };
        cells.fill(BLANK);
        self.cur_col = 0;
        self.cur_row = 0;
    }

    /// Clears only the scrolling log area, keeping the logo and status bar.
    pub fn clear_log(&mut self) {
        // SAFETY: single-threaded (interrupts off in callers); no other reference to the grid is live.
        let cells = unsafe { CELLS.get() };
        let top = self.first_row * MAX_COLS;
        cells[top..top + self.log_rows * MAX_COLS].fill(BLANK);
        self.cur_col = 0;
        self.cur_row = 0;
        self.redraw_log();
    }

    pub fn draw_logo(&self) {
        self.draw_logo_rgb(LOGO_RGB, BG);
    }

    fn draw_logo_rgb(&self, fg: (u8, u8, u8), bg: (u8, u8, u8)) {
        if self.cols < BANNER_COLS || self.rows < BANNER_ROWS {
            return;
        }
        let start_col = (self.cols - BANNER_COLS) / 2;
        for (r, line) in BANNER.iter().enumerate() {
            for (c, &ch) in line.iter().enumerate() {
                if ch != b' ' {
                    self.draw_glyph(start_col + c, self.logo_row + r, ch, fg, bg);
                }
            }
        }
    }

    // ---- splash: logo, fade-in and progress bar instead of the scrolling log ----

    /// Bar rectangle `(x, y, width, height)` in pixels, centred under the logo.
    fn bar_rect(&self) -> (usize, usize, usize, usize) {
        let w = (240 * self.scale).clamp(160, self.width.saturating_sub(40));
        let x = (self.width - w) / 2;
        let y = (self.logo_row + BANNER_ROWS + 2) * self.cell_h();
        let h = (4 * self.scale).max(4);
        (x, y, w, h)
    }

    /// Draws a sleek rounded pill shape for the progress bar and track.
    fn draw_pill(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        if w == 0 || h == 0 {
            return;
        }
        for yy in 0..h {
            let indent = if h >= 6 {
                if yy == 0 || yy == h - 1 { 2 } else if yy == 1 || yy == h - 2 { 1 } else { 0 }
            } else if h >= 3 {
                if yy == 0 || yy == h - 1 { 1 } else { 0 }
            } else {
                0
            };
            if indent * 2 >= w {
                continue;
            }
            if let Some(row) = self.row_mut(x + indent, y + yy, w - indent * 2) {
                row.fill(color);
            }
        }
    }

    /// Clears the screen, centres the logo and draws the empty progress track. Log output is held
    /// back (it stays in the ring buffer and on serial) until `end_splash`.
    pub fn begin_splash(&mut self) {
        self.clear();
        self.splash = true;
        self.logo_row = splash::splash_row(self.rows, BANNER_ROWS);
        let (x, y, w, h) = self.bar_rect();
        let track = self.px(TRACK_RGB);
        self.draw_pill(x, y, w, h, track);
    }

    pub fn splash_active(&self) -> bool {
        self.splash
    }

    /// Draws the logo at brightness `level` (0 = black, 255 = full), for a fade-in.
    pub fn fade_logo(&self, level: u8) {
        self.draw_logo_rgb(splash::dim(LOGO_RGB, level), BG);
    }

    /// Fills the progress bar to `permille` thousandths.
    pub fn progress(&mut self, permille: u32) {
        if !self.splash {
            return;
        }
        self.bar_pm = permille.min(1000);
        let (x, y, w, h) = self.bar_rect();
        let fg = self.px(LOGO_RGB);
        let filled = splash::fill_width(w, self.bar_pm);
        self.draw_pill(x, y, filled, h, fg);
    }

    /// Where the bar currently stands, in thousandths.
    pub fn bar(&self) -> u32 {
        self.bar_pm
    }

    /// Dims the whole splash (logo, bar track and bar fill) to `level` / 255 for the fade-out.
    pub fn fade_splash(&self, level: u8) {
        if !self.splash {
            return;
        }
        self.draw_logo_rgb(splash::dim(LOGO_RGB, level), BG);
        let (x, y, w, h) = self.bar_rect();
        let track = self.px(splash::dim(TRACK_RGB, level));
        let fg = self.px(splash::dim(LOGO_RGB, level));
        let filled = splash::fill_width(w, self.bar_pm);
        self.draw_pill(x, y, w, h, track);
        self.draw_pill(x, y, filled, h, fg);
    }

    /// Shows a short centred line under the bar (empty `text` removes it).
    pub fn hint(&self, text: &[u8]) {
        if !self.splash {
            return;
        }
        let row = self.logo_row + BANNER_ROWS + 4;
        if row >= self.rows {
            return;
        }
        let n = text.len().min(self.cols);
        if n == 0 {
            self.fill_rect(0, row * self.cell_h(), self.width, self.cell_h(), self.px(BG));
            return;
        }
        let col = (self.cols - n) / 2;
        for (i, &b) in text[..n].iter().enumerate() {
            self.draw_glyph(col + i, row, b, (120, 130, 145), BG);
        }
    }

    /// Leaves the splash: the screen returns to logo at the top, log area and status bar.
    pub fn end_splash(&mut self) {
        if !self.splash {
            return;
        }
        self.splash = false;
        self.clear();
        self.logo_row = self.rows.saturating_sub(BANNER_ROWS) / 3;
        self.draw_logo();
    }

    /// Full-screen stop screen for an unrecoverable error. Shows only what the user needs: where it
    /// failed and what to do. No addresses or register values are printed to the screen.
    pub fn panic_screen(&mut self, lines: &[&[u8]]) {
        self.splash = false;
        let bg = self.px(PANIC_BG);
        self.fill_rect(0, 0, self.width, self.height, bg);
        self.logo_row = self.rows.saturating_sub(BANNER_ROWS + 10) / 3;
        self.draw_logo_rgb(PANIC_LOGO, PANIC_BG);
        let mut row = self.logo_row + BANNER_ROWS + 2;
        let put = |this: &Self, row: usize, text: &[u8], fg: (u8, u8, u8)| {
            if row >= this.rows {
                return;
            }
            let n = text.len().min(this.cols);
            let col = (this.cols - n) / 2;
            for (i, &b) in text[..n].iter().enumerate() {
                this.draw_glyph(col + i, row, b, fg, PANIC_BG);
            }
        };
        put(self, row, b"AEON STOPPED", (255, 200, 200));
        row += 2;
        for l in lines {
            put(self, row, l, (230, 190, 190));
            row += 1;
        }
        put(self, row + 1, b"The system is halted. Power off or press reset to restart.", (160, 120, 120));
    }

    fn draw_glyph(&self, col: usize, row: usize, ch: u8, fg: (u8, u8, u8), bg: (u8, u8, u8)) {
        let g = if (font::FIRST..=font::LAST).contains(&ch) {
            &font::GLYPHS[(ch - font::FIRST) as usize]
        } else {
            &font::GLYPHS[(b'?' - font::FIRST) as usize]
        };
        let (fgp, bgp) = (self.px(fg), self.px(bg));
        let s = self.scale;
        let (x0, y0) = (col * self.cell_w(), row * self.cell_h());
        let mut line = [0u32; 8 * MAX_SCALE];
        for (fy, bits) in g.iter().enumerate() {
            for fx in 0..8 {
                let v = if bits & (1 << fx) != 0 { fgp } else { bgp };
                line[fx * s..(fx + 1) * s].fill(v);
            }
            // Each font row is 2 * scale scan lines tall (8x8 glyphs on 8x16 cells).
            for sy in 0..2 * s {
                if let Some(dst) = self.row_mut(x0, y0 + fy * 2 * s + sy, 8 * s) {
                    let n = dst.len();
                    dst.copy_from_slice(&line[..n]);
                }
            }
        }
    }

    fn redraw_log(&self) {
        let bg = self.px(BG);
        self.fill_rect(0, self.first_row * self.cell_h(), self.width, self.log_rows * self.cell_h(), bg);
        // SAFETY: single-threaded; the grid is only read here and no mutable reference is live.
        let cells = unsafe { &*CELLS.as_ptr() };
        for r in 0..self.log_rows {
            for c in 0..self.cols {
                let cell = cells[(self.first_row + r) * MAX_COLS + c];
                if !matches!(cell.ch, 0 | b' ') {
                    self.draw_glyph(c, self.first_row + r, cell.ch, PALETTE[cell.color as usize % PALETTE.len()], BG);
                }
            }
        }
    }

    fn newline(&mut self) {
        self.cur_col = 0;
        if self.cur_row + 1 < self.log_rows {
            self.cur_row += 1;
            return;
        }
        // Scroll the log area up one row inside the cell grid, then repaint it.
        // SAFETY: single-threaded; no other reference to the grid is live.
        let cells = unsafe { CELLS.get() };
        let top = self.first_row * MAX_COLS;
        let end = (self.first_row + self.log_rows) * MAX_COLS;
        cells.copy_within(top + MAX_COLS..end, top);
        cells[end - MAX_COLS..end].fill(BLANK);
        self.redraw_log();
    }

    fn put_byte(&mut self, b: u8, color: u8) {
        match b {
            b'\n' => self.newline(),
            b'\r' => self.cur_col = 0,
            0x08 => self.cur_col = self.cur_col.saturating_sub(1),
            _ => {
                if self.cur_col >= self.cols {
                    self.newline();
                }
                let row = self.first_row + self.cur_row;
                // SAFETY: single-threaded; no other reference to the grid is live.
                let cells = unsafe { CELLS.get() };
                cells[row * MAX_COLS + self.cur_col] = Cell { ch: b, color };
                self.draw_glyph(self.cur_col, row, b, PALETTE[color as usize % PALETTE.len()], BG);
                self.cur_col += 1;
            }
        }
    }

    pub fn write_line(&mut self, level: Level, bytes: &[u8]) {
        if self.splash {
            return;
        }
        let color = color_for(level);
        for &b in bytes {
            self.put_byte(b, color);
        }
    }

    /// Repaints the bottom row with `text` (clipped or padded to the screen width).
    pub fn status(&self, text: &[u8]) {
        let row = self.rows - 1;
        for c in 0..self.cols {
            let ch = text.get(c).copied().unwrap_or(b' ');
            self.draw_glyph(c, row, ch, BAR_FG, BAR_BG);
        }
        // Colour the leftover pixels at the right edge as well.
        let used = self.cols * self.cell_w();
        let bar = self.px(BAR_BG);
        self.fill_rect(used, row * self.cell_h(), self.width.saturating_sub(used), self.cell_h(), bar);
    }
}
