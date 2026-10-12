//! Text editors
//!
//! Minimal vi/vim implementation and hexedit.

#[cfg(all(feature = "vi", feature = "alloc"))]
use alloc::vec::Vec;

use super::get_arg;
use crate::io;

/// vi - screen-oriented text editor
///
/// A minimal but functional vi implementation supporting:
/// - Normal mode: h/j/k/l navigation, x (delete), dd (delete line), yy (yank), p (paste)
/// - Insert mode: i (insert), a (append), o (open line)
/// - Command mode: :w (write), :q (quit), :wq, :q!
/// - Search: / and n for next
#[cfg(all(feature = "vi", feature = "alloc"))]
pub fn vi(argc: i32, argv: *const *const u8) -> i32 {
    let filename = if argc > 1 {
        unsafe { get_arg(argv, 1) }
    } else {
        None
    };

    let mut editor = Editor::new();

    // Load file if specified
    if let Some(path) = filename {
        editor.filename = Some(path.to_vec());
        let fd = io::open(path, libc::O_RDONLY, 0);
        if fd >= 0 {
            let content = io::read_all(fd);
            io::close(fd);
            editor.load_content(&content);
        }
    }

    // Ensure at least one line
    if editor.lines.is_empty() {
        editor.lines.push(Vec::new());
    }

    // Enter raw terminal mode
    let mut orig_termios: libc::termios = unsafe { core::mem::zeroed() };
    unsafe {
        libc::tcgetattr(0, &mut orig_termios);
        let mut raw = orig_termios;
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
        raw.c_iflag &= !(libc::IXON | libc::ICRNL | libc::BRKINT | libc::INPCK | libc::ISTRIP);
        raw.c_oflag &= !libc::OPOST;
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        libc::tcsetattr(0, libc::TCSAFLUSH, &raw);
    }

    // Get terminal size
    let mut ws: libc::winsize = unsafe { core::mem::zeroed() };
    unsafe {
        libc::ioctl(1, libc::TIOCGWINSZ, &mut ws);
    }
    editor.rows = if ws.ws_row > 2 {
        ws.ws_row as usize - 2
    } else {
        22
    };
    editor.cols = if ws.ws_col > 0 {
        ws.ws_col as usize
    } else {
        80
    };

    // Main loop
    editor.refresh_screen();

    loop {
        let c = read_key();

        match editor.mode {
            Mode::Normal => {
                if !editor.handle_normal_key(c) {
                    break;
                }
            }
            Mode::Insert => {
                editor.handle_insert_key(c);
            }
            Mode::Command => {
                editor.handle_command_key(c);
                if editor.should_quit {
                    break;
                }
            }
        }

        editor.refresh_screen();
    }

    // Restore terminal
    unsafe {
        libc::tcsetattr(0, libc::TCSAFLUSH, &orig_termios);
    }

    // Clear screen on exit
    io::write_str(1, b"\x1b[2J\x1b[H");

    0
}

#[cfg(all(feature = "vi", not(feature = "alloc")))]
pub fn vi(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"vi: requires alloc feature\n");
    1
}

/// view - read-only vi
#[cfg(feature = "vi")]
pub fn view(argc: i32, argv: *const *const u8) -> i32 {
    vi(argc, argv)
}

/// hexedit - hex editor
///
/// Simple hex editor for binary files.
#[cfg(all(feature = "hexedit", feature = "alloc"))]
pub fn hexedit(argc: i32, argv: *const *const u8) -> i32 {
    let filename = if argc > 1 {
        match unsafe { get_arg(argv, 1) } {
            Some(f) => f,
            None => {
                io::write_str(2, b"hexedit: missing filename\n");
                return 1;
            }
        }
    } else {
        io::write_str(2, b"Usage: hexedit <file>\n");
        return 1;
    };

    let fd = io::open(filename, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"hexedit: cannot open file\n");
        return 1;
    }

    let content = io::read_all(fd);
    io::close(fd);

    let mut editor = HexEditor::new(content, filename.to_vec());

    // Enter raw mode
    let mut orig_termios: libc::termios = unsafe { core::mem::zeroed() };
    unsafe {
        libc::tcgetattr(0, &mut orig_termios);
        let mut raw = orig_termios;
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG);
        raw.c_iflag &= !(libc::IXON | libc::ICRNL);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        libc::tcsetattr(0, libc::TCSAFLUSH, &raw);
    }

    // Get terminal size
    let mut ws: libc::winsize = unsafe { core::mem::zeroed() };
    unsafe {
        libc::ioctl(1, libc::TIOCGWINSZ, &mut ws);
    }
    editor.rows = if ws.ws_row > 2 {
        ws.ws_row as usize - 2
    } else {
        22
    };

    // Main loop
    editor.refresh();

    loop {
        let c = read_key();
        if !editor.handle_key(c) {
            break;
        }
        editor.refresh();
    }

    // Restore terminal
    unsafe {
        libc::tcsetattr(0, libc::TCSAFLUSH, &orig_termios);
    }

    io::write_str(1, b"\x1b[2J\x1b[H");
    0
}

#[cfg(all(feature = "hexedit", not(feature = "alloc")))]
pub fn hexedit(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"hexedit: requires alloc feature\n");
    1
}

// ============================================================================
// Vi Editor Implementation
// ============================================================================

#[cfg(all(feature = "vi", feature = "alloc"))]
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    Insert,
    Command,
}

#[cfg(all(feature = "vi", feature = "alloc"))]
struct Editor {
    lines: Vec<Vec<u8>>,
    cursor_x: usize,
    cursor_y: usize,
    offset_y: usize,
    mode: Mode,
    filename: Option<Vec<u8>>,
    modified: bool,
    command_buf: Vec<u8>,
    yank_buf: Vec<u8>,
    search_pattern: Vec<u8>,
    message: Vec<u8>,
    rows: usize,
    cols: usize,
    should_quit: bool,
}

#[cfg(all(feature = "vi", feature = "alloc"))]
impl Editor {
    fn new() -> Self {
        Editor {
            lines: Vec::new(),
            cursor_x: 0,
            cursor_y: 0,
            offset_y: 0,
            mode: Mode::Normal,
            filename: None,
            modified: false,
            command_buf: Vec::new(),
            yank_buf: Vec::new(),
            search_pattern: Vec::new(),
            message: Vec::new(),
            rows: 24,
            cols: 80,
            should_quit: false,
        }
    }

    fn load_content(&mut self, content: &[u8]) {
        self.lines = content.split(|&c| c == b'\n').map(|l| l.to_vec()).collect();

        // Remove trailing empty line if content ended with newline
        if self.lines.len() > 1 && self.lines.last().map(|l| l.is_empty()).unwrap_or(false) {
            self.lines.pop();
        }
    }

    fn current_line(&self) -> &[u8] {
        if self.cursor_y < self.lines.len() {
            &self.lines[self.cursor_y]
        } else {
            b""
        }
    }

    fn current_line_mut(&mut self) -> &mut Vec<u8> {
        while self.cursor_y >= self.lines.len() {
            self.lines.push(Vec::new());
        }
        &mut self.lines[self.cursor_y]
    }

    fn refresh_screen(&self) {
        let mut buf = Vec::with_capacity(4096);

        // Hide cursor and go to top
        buf.extend_from_slice(b"\x1b[?25l\x1b[H");

        // Draw lines
        for i in 0..self.rows {
            let file_row = self.offset_y + i;

            if file_row < self.lines.len() {
                let line = &self.lines[file_row];
                let len = core::cmp::min(line.len(), self.cols);
                buf.extend_from_slice(&line[..len]);
            } else {
                buf.push(b'~');
            }

            // Clear to end of line and newline
            buf.extend_from_slice(b"\x1b[K\r\n");
        }

        // Status line
        buf.extend_from_slice(b"\x1b[7m"); // Reverse video

        // Mode indicator
        let mode_str = match self.mode {
            Mode::Normal => b"-- NORMAL --" as &[u8],
            Mode::Insert => b"-- INSERT --",
            Mode::Command => b":",
        };
        buf.extend_from_slice(mode_str);

        if self.mode == Mode::Command {
            buf.extend_from_slice(&self.command_buf);
        }

        // Filename and modified indicator
        buf.extend_from_slice(b"  ");
        if let Some(ref name) = self.filename {
            let len = core::cmp::min(name.len(), 20);
            buf.extend_from_slice(&name[..len]);
        } else {
            buf.extend_from_slice(b"[No Name]");
        }
        if self.modified {
            buf.extend_from_slice(b" [+]");
        }

        // Pad status line
        let status_len = buf.len();
        for _ in status_len..self.cols + 100 {
            buf.push(b' ');
        }
        buf.extend_from_slice(b"\x1b[m\r\n"); // Reset attributes

        // Message line
        buf.extend_from_slice(&self.message);
        buf.extend_from_slice(b"\x1b[K");

        // Position cursor
        let screen_y = self.cursor_y - self.offset_y + 1;
        let screen_x = self.cursor_x + 1;
        buf.extend_from_slice(b"\x1b[");
        append_num(&mut buf, screen_y);
        buf.push(b';');
        append_num(&mut buf, screen_x);
        buf.push(b'H');

        // Show cursor
        buf.extend_from_slice(b"\x1b[?25h");

        io::write_all(1, &buf);
    }

    fn handle_normal_key(&mut self, c: u8) -> bool {
        self.message.clear();

        match c {
            b'h' | 0x44 => {
                // Left
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
            }
            b'l' | 0x43 => {
                // Right
                let line_len = self.current_line().len();
                if self.cursor_x < line_len.saturating_sub(1) {
                    self.cursor_x += 1;
                }
            }
            b'j' | 0x42 => {
                // Down
                if self.cursor_y < self.lines.len().saturating_sub(1) {
                    self.cursor_y += 1;
                    self.adjust_cursor_x();
                    self.scroll_if_needed();
                }
            }
            b'k' | 0x41 => {
                // Up
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    self.adjust_cursor_x();
                    self.scroll_if_needed();
                }
            }
            b'0' => {
                self.cursor_x = 0;
            }
            b'$' => {
                let len = self.current_line().len();
                self.cursor_x = if len > 0 { len - 1 } else { 0 };
            }
            b'g' => {
                // gg - go to beginning
                self.cursor_y = 0;
                self.cursor_x = 0;
                self.offset_y = 0;
            }
            b'G' => {
                // Go to end
                self.cursor_y = self.lines.len().saturating_sub(1);
                self.scroll_if_needed();
            }
            b'i' => {
                self.mode = Mode::Insert;
            }
            b'a' => {
                let line_len = self.current_line().len();
                if line_len > 0 {
                    self.cursor_x = core::cmp::min(self.cursor_x + 1, line_len);
                }
                self.mode = Mode::Insert;
            }
            b'A' => {
                self.cursor_x = self.current_line().len();
                self.mode = Mode::Insert;
            }
            b'o' => {
                self.cursor_y += 1;
                self.lines.insert(self.cursor_y, Vec::new());
                self.cursor_x = 0;
                self.modified = true;
                self.mode = Mode::Insert;
            }
            b'O' => {
                self.lines.insert(self.cursor_y, Vec::new());
                self.cursor_x = 0;
                self.modified = true;
                self.mode = Mode::Insert;
            }
            b'x' => {
                let cursor_x = self.cursor_x;
                if !self.current_line().is_empty() && cursor_x < self.lines[self.cursor_y].len() {
                    self.lines[self.cursor_y].remove(cursor_x);
                    self.modified = true;
                    let line_len = self.lines[self.cursor_y].len();
                    if self.cursor_x >= line_len && self.cursor_x > 0 {
                        self.cursor_x -= 1;
                    }
                }
            }
            b'd' => {
                // dd - delete line (simplified: just delete on 'd')
                if !self.lines.is_empty() {
                    self.yank_buf = self.lines[self.cursor_y].clone();
                    self.lines.remove(self.cursor_y);
                    if self.lines.is_empty() {
                        self.lines.push(Vec::new());
                    }
                    if self.cursor_y >= self.lines.len() {
                        self.cursor_y = self.lines.len() - 1;
                    }
                    self.adjust_cursor_x();
                    self.modified = true;
                }
            }
            b'y' => {
                // yy - yank line
                self.yank_buf = self.current_line().to_vec();
                self.message = b"1 line yanked".to_vec();
            }
            b'p' => {
                // Paste after
                if !self.yank_buf.is_empty() {
                    self.cursor_y += 1;
                    self.lines.insert(self.cursor_y, self.yank_buf.clone());
                    self.cursor_x = 0;
                    self.modified = true;
                }
            }
            b'P' => {
                // Paste before
                if !self.yank_buf.is_empty() {
                    self.lines.insert(self.cursor_y, self.yank_buf.clone());
                    self.cursor_x = 0;
                    self.modified = true;
                }
            }
            b':' => {
                self.mode = Mode::Command;
                self.command_buf.clear();
            }
            b'/' => {
                self.mode = Mode::Command;
                self.command_buf.clear();
                self.command_buf.push(b'/');
            }
            b'n' => {
                // Search next
                self.search_next();
            }
            27 => { // Escape
                // Already in normal mode
            }
            _ => {}
        }

        true
    }

    fn handle_insert_key(&mut self, c: u8) {
        match c {
            27 => {
                // Escape
                self.mode = Mode::Normal;
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
            }
            127 | 8 => {
                // Backspace
                let cursor_x = self.cursor_x;
                let cursor_y = self.cursor_y;
                if cursor_x > 0 {
                    self.lines[cursor_y].remove(cursor_x - 1);
                    self.cursor_x -= 1;
                    self.modified = true;
                } else if cursor_y > 0 {
                    // Join with previous line
                    let current = self.lines.remove(cursor_y);
                    self.cursor_y -= 1;
                    self.cursor_x = self.lines[self.cursor_y].len();
                    self.lines[self.cursor_y].extend_from_slice(&current);
                    self.modified = true;
                }
            }
            13 | 10 => {
                // Enter
                let cursor_x = self.cursor_x;
                let cursor_y = self.cursor_y;
                let rest = self.lines[cursor_y][cursor_x..].to_vec();
                self.lines[cursor_y].truncate(cursor_x);
                self.cursor_y += 1;
                self.cursor_x = 0;
                self.lines.insert(self.cursor_y, rest);
                self.modified = true;
                self.scroll_if_needed();
            }
            0x1b => { // Start of escape sequence - ignore
            }
            c if c >= 32 && c < 127 => {
                let cursor_x = self.cursor_x;
                let cursor_y = self.cursor_y;
                let line_len = self.lines[cursor_y].len();
                if cursor_x >= line_len {
                    self.lines[cursor_y].push(c);
                } else {
                    self.lines[cursor_y].insert(cursor_x, c);
                }
                self.cursor_x += 1;
                self.modified = true;
            }
            _ => {}
        }
    }

    fn handle_command_key(&mut self, c: u8) {
        match c {
            27 => {
                // Escape
                self.mode = Mode::Normal;
                self.command_buf.clear();
            }
            13 | 10 => {
                // Enter
                self.execute_command();
                self.mode = Mode::Normal;
            }
            127 | 8 => {
                // Backspace
                self.command_buf.pop();
                if self.command_buf.is_empty() {
                    self.mode = Mode::Normal;
                }
            }
            c if c >= 32 && c < 127 => {
                self.command_buf.push(c);
            }
            _ => {}
        }
    }

    fn execute_command(&mut self) {
        let cmd = &self.command_buf;

        if cmd.starts_with(b"/") {
            // Search
            self.search_pattern = cmd[1..].to_vec();
            self.search_next();
        } else if cmd == b"w" || cmd == b"write" {
            self.save();
        } else if cmd == b"q" || cmd == b"quit" {
            if self.modified {
                self.message = b"No write since last change (use :q! to override)".to_vec();
            } else {
                self.should_quit = true;
            }
        } else if cmd == b"q!" {
            self.should_quit = true;
        } else if cmd == b"wq" || cmd == b"x" {
            self.save();
            self.should_quit = true;
        } else if cmd.starts_with(b"w ") {
            self.filename = Some(cmd[2..].to_vec());
            self.save();
        } else {
            self.message = b"Unknown command".to_vec();
        }

        self.command_buf.clear();
    }

    fn save(&mut self) {
        let path = match &self.filename {
            Some(p) => p.clone(),
            None => {
                self.message = b"No file name".to_vec();
                return;
            }
        };

        let fd = io::open(&path, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
        if fd < 0 {
            self.message = b"Cannot write file".to_vec();
            return;
        }

        let mut bytes = 0;
        for (i, line) in self.lines.iter().enumerate() {
            io::write_all(fd, line);
            bytes += line.len();
            if i < self.lines.len() - 1 {
                io::write_str(fd, b"\n");
                bytes += 1;
            }
        }
        io::close(fd);

        self.modified = false;
        self.message = format_write_msg(bytes, self.lines.len());
    }

    fn search_next(&mut self) {
        if self.search_pattern.is_empty() {
            return;
        }

        let start_y = self.cursor_y;
        let start_x = self.cursor_x + 1;

        // Search from current position
        for i in 0..self.lines.len() {
            let y = (start_y + i) % self.lines.len();
            let line = &self.lines[y];
            let search_start = if i == 0 { start_x } else { 0 };

            if let Some(pos) = find_pattern(line, &self.search_pattern, search_start) {
                self.cursor_y = y;
                self.cursor_x = pos;
                self.scroll_if_needed();
                return;
            }
        }

        self.message = b"Pattern not found".to_vec();
    }

    fn adjust_cursor_x(&mut self) {
        let line_len = self.current_line().len();
        if self.cursor_x >= line_len {
            self.cursor_x = if line_len > 0 { line_len - 1 } else { 0 };
        }
    }

    fn scroll_if_needed(&mut self) {
        if self.cursor_y < self.offset_y {
            self.offset_y = self.cursor_y;
        }
        if self.cursor_y >= self.offset_y + self.rows {
            self.offset_y = self.cursor_y - self.rows + 1;
        }
    }
}

// ============================================================================
// Hex Editor Implementation
// ============================================================================

#[cfg(all(feature = "hexedit", feature = "alloc"))]
struct HexEditor {
    data: Vec<u8>,
    filename: Vec<u8>,
    cursor: usize,
    offset: usize,
    modified: bool,
    rows: usize,
}

#[cfg(all(feature = "hexedit", feature = "alloc"))]
impl HexEditor {
    fn new(data: Vec<u8>, filename: Vec<u8>) -> Self {
        HexEditor {
            data,
            filename,
            cursor: 0,
            offset: 0,
            modified: false,
            rows: 20,
        }
    }

    fn refresh(&self) {
        let bytes_per_row = 16;
        let mut buf = Vec::with_capacity(4096);

        buf.extend_from_slice(b"\x1b[H\x1b[J"); // Clear screen

        for row in 0..self.rows {
            let addr = (self.offset + row) * bytes_per_row;
            if addr >= self.data.len() {
                break;
            }

            // Address
            let addr_str = format_hex_addr(addr);
            buf.extend_from_slice(&addr_str);
            buf.extend_from_slice(b"  ");

            // Hex bytes
            for col in 0..bytes_per_row {
                let pos = addr + col;
                if pos < self.data.len() {
                    if pos == self.cursor {
                        buf.extend_from_slice(b"\x1b[7m"); // Reverse
                    }
                    let hex = format_hex_byte(self.data[pos]);
                    buf.extend_from_slice(&hex);
                    if pos == self.cursor {
                        buf.extend_from_slice(b"\x1b[m"); // Reset
                    }
                    buf.push(b' ');
                } else {
                    buf.extend_from_slice(b"   ");
                }
                if col == 7 {
                    buf.push(b' ');
                }
            }

            buf.extend_from_slice(b" |");

            // ASCII
            for col in 0..bytes_per_row {
                let pos = addr + col;
                if pos < self.data.len() {
                    let c = self.data[pos];
                    if c >= 32 && c < 127 {
                        buf.push(c);
                    } else {
                        buf.push(b'.');
                    }
                }
            }

            buf.extend_from_slice(b"|\r\n");
        }

        // Status line
        buf.extend_from_slice(b"\x1b[7m ");
        buf.extend_from_slice(&self.filename);
        if self.modified {
            buf.extend_from_slice(b" [modified]");
        }
        buf.extend_from_slice(b" - ");
        let pos_str = format_hex_addr(self.cursor);
        buf.extend_from_slice(&pos_str);
        buf.extend_from_slice(b" \x1b[m");

        io::write_all(1, &buf);
    }

    fn handle_key(&mut self, c: u8) -> bool {
        match c {
            b'q' | 27 => return false,
            b'h' | 0x44 => {
                // Left
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.scroll_if_needed();
                }
            }
            b'l' | 0x43 => {
                // Right
                if self.cursor < self.data.len().saturating_sub(1) {
                    self.cursor += 1;
                    self.scroll_if_needed();
                }
            }
            b'j' | 0x42 => {
                // Down
                if self.cursor + 16 < self.data.len() {
                    self.cursor += 16;
                    self.scroll_if_needed();
                }
            }
            b'k' | 0x41 => {
                // Up
                if self.cursor >= 16 {
                    self.cursor -= 16;
                    self.scroll_if_needed();
                }
            }
            b'w' => {
                // Write file
                let fd = io::open(
                    &self.filename,
                    libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                    0o644,
                );
                if fd >= 0 {
                    io::write_all(fd, &self.data);
                    io::close(fd);
                    self.modified = false;
                }
            }
            c if (c >= b'0' && c <= b'9') || (c >= b'a' && c <= b'f') => {
                // Edit hex digit
                let val = if c >= b'a' { c - b'a' + 10 } else { c - b'0' };
                if self.cursor < self.data.len() {
                    self.data[self.cursor] = (self.data[self.cursor] & 0x0F) << 4 | val;
                    self.modified = true;
                }
            }
            _ => {}
        }
        true
    }

    fn scroll_if_needed(&mut self) {
        let row = self.cursor / 16;
        if row < self.offset {
            self.offset = row;
        }
        if row >= self.offset + self.rows {
            self.offset = row - self.rows + 1;
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Read a key from stdin
#[cfg(any(
    all(feature = "vi", feature = "alloc"),
    all(feature = "hexedit", feature = "alloc")
))]
fn read_key() -> u8 {
    let mut c = [0u8; 1];
    let n = io::read(0, &mut c);
    if n <= 0 {
        return 0;
    }

    // Handle escape sequences
    if c[0] == 27 {
        let mut seq = [0u8; 2];
        if io::read(0, &mut seq[0..1]) <= 0 {
            return 27;
        }
        if io::read(0, &mut seq[1..2]) <= 0 {
            return 27;
        }

        if seq[0] == b'[' {
            // Arrow keys: [A, [B, [C, [D
            return seq[1];
        }
        return 27;
    }

    c[0]
}

#[cfg(all(feature = "vi", feature = "alloc"))]
fn append_num(buf: &mut Vec<u8>, n: usize) {
    let mut tmp = [0u8; 16];
    let s = crate::sys::format_u64(n as u64, &mut tmp);
    buf.extend_from_slice(s);
}

#[cfg(all(feature = "vi", feature = "alloc"))]
fn format_write_msg(bytes: usize, lines: usize) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.push(b'"');
    // Would add filename here
    msg.extend_from_slice(b"\" ");
    let mut tmp = [0u8; 16];
    msg.extend_from_slice(crate::sys::format_u64(lines as u64, &mut tmp));
    msg.extend_from_slice(b"L, ");
    msg.extend_from_slice(crate::sys::format_u64(bytes as u64, &mut tmp));
    msg.extend_from_slice(b"B written");
    msg
}

#[cfg(all(feature = "vi", feature = "alloc"))]
fn find_pattern(line: &[u8], pattern: &[u8], start: usize) -> Option<usize> {
    if pattern.is_empty() || start + pattern.len() > line.len() {
        return None;
    }
    for i in start..=line.len() - pattern.len() {
        if &line[i..i + pattern.len()] == pattern {
            return Some(i);
        }
    }
    None
}

#[cfg(any(
    all(feature = "vi", feature = "alloc"),
    all(feature = "hexedit", feature = "alloc")
))]
fn format_hex_addr(addr: usize) -> [u8; 8] {
    let mut result = [b'0'; 8];
    let mut tmp = [0u8; 16];
    let hex = crate::sys::format_hex(addr as u64, &mut tmp);
    let start = 8usize.saturating_sub(hex.len());
    for (i, &b) in hex.iter().enumerate() {
        if start + i < 8 {
            result[start + i] = b;
        }
    }
    result
}

#[cfg(all(feature = "hexedit", feature = "alloc"))]
fn format_hex_byte(b: u8) -> [u8; 2] {
    const HEX: &[u8] = b"0123456789abcdef";
    [HEX[(b >> 4) as usize], HEX[(b & 0xF) as usize]]
}
