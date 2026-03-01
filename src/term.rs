use std::io::{self, Read, Write};

// ─── ANSI Escape Constants ───────────────────────────────────────────────────

pub const ALT_SCREEN_ON: &str = "\x1b[?1049h";
pub const ALT_SCREEN_OFF: &str = "\x1b[?1049l";
pub const CURSOR_HIDE: &str = "\x1b[?25l";
pub const CURSOR_SHOW: &str = "\x1b[?25h";
#[allow(dead_code)]
pub const CLEAR_SCREEN: &str = "\x1b[2J";
pub const CURSOR_HOME: &str = "\x1b[H";
pub const CLEAR_TO_EOL: &str = "\x1b[K";
pub const CLEAR_TO_EOS: &str = "\x1b[0J";
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";

fn fg_color(ansi256: u8) -> String {
    format!("\x1b[38;5;{}m", ansi256)
}

#[allow(dead_code)]
fn bg_color(ansi256: u8) -> String {
    format!("\x1b[48;5;{}m", ansi256)
}

pub fn style_red(text: &str) -> String {
    format!("{}{}{}", fg_color(9), text, RESET)
}

pub fn style_green(text: &str) -> String {
    format!("{}{}{}", fg_color(10), text, RESET)
}

pub fn style_cyan_bold(text: &str) -> String {
    format!("{}{}{}{}", BOLD, fg_color(6), text, RESET)
}

pub fn style_grey(text: &str) -> String {
    format!("{}{}{}", fg_color(240), text, RESET)
}

pub fn style_red_bold(text: &str) -> String {
    format!("{}{}{}{}", BOLD, fg_color(9), text, RESET)
}

pub fn style_bold(text: &str) -> String {
    format!("{}{}{}", BOLD, text, RESET)
}

// ─── Key Enum ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Space,
    CtrlC,
    Unknown,
}

// ─── Platform: Windows ───────────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use std::sync::Mutex;

    static ORIG_INPUT_MODE: Mutex<Option<u32>> = Mutex::new(None);
    static ORIG_OUTPUT_MODE: Mutex<Option<u32>> = Mutex::new(None);

    const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6; // -10 as u32
    const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5; // -11 as u32

    const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
    const ENABLE_LINE_INPUT: u32 = 0x0002;
    const ENABLE_ECHO_INPUT: u32 = 0x0004;
    const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;

    extern "system" {
        fn GetConsoleMode(handle: *mut std::ffi::c_void, mode: *mut u32) -> i32;
        fn SetConsoleMode(handle: *mut std::ffi::c_void, mode: u32) -> i32;
        fn GetStdHandle(std_handle: u32) -> *mut std::ffi::c_void;
        fn GetConsoleScreenBufferInfo(
            handle: *mut std::ffi::c_void,
            info: *mut ConsoleScreenBufferInfo,
        ) -> i32;
    }

    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    struct SmallRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }

    #[repr(C)]
    struct ConsoleScreenBufferInfo {
        size: Coord,
        cursor_position: Coord,
        attributes: u16,
        window: SmallRect,
        maximum_window_size: Coord,
    }

    pub fn enable_raw_mode() {
        unsafe {
            let stdin_handle = GetStdHandle(STD_INPUT_HANDLE);
            let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);

            // Save original input mode
            let mut input_mode: u32 = 0;
            GetConsoleMode(stdin_handle, &mut input_mode);
            *ORIG_INPUT_MODE.lock().unwrap() = Some(input_mode);

            // Save original output mode
            let mut output_mode: u32 = 0;
            GetConsoleMode(stdout_handle, &mut output_mode);
            *ORIG_OUTPUT_MODE.lock().unwrap() = Some(output_mode);

            // Set raw input mode
            let new_input = (input_mode
                & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT))
                | ENABLE_VIRTUAL_TERMINAL_INPUT;
            SetConsoleMode(stdin_handle, new_input);

            // Enable VT processing on stdout
            let new_output =
                output_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING | ENABLE_PROCESSED_OUTPUT;
            SetConsoleMode(stdout_handle, new_output);
        }
    }

    pub fn disable_raw_mode() {
        unsafe {
            if let Some(mode) = *ORIG_INPUT_MODE.lock().unwrap() {
                let handle = GetStdHandle(STD_INPUT_HANDLE);
                SetConsoleMode(handle, mode);
            }
            if let Some(mode) = *ORIG_OUTPUT_MODE.lock().unwrap() {
                let handle = GetStdHandle(STD_OUTPUT_HANDLE);
                SetConsoleMode(handle, mode);
            }
        }
    }

    pub fn terminal_size() -> (u16, u16) {
        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            let mut info = std::mem::zeroed::<ConsoleScreenBufferInfo>();
            if GetConsoleScreenBufferInfo(handle, &mut info) != 0 {
                let cols = (info.window.right - info.window.left + 1) as u16;
                let rows = (info.window.bottom - info.window.top + 1) as u16;
                (cols, rows)
            } else {
                (80, 24)
            }
        }
    }
}

#[cfg(unix)]
mod platform {
    use std::sync::Mutex;
    use std::os::raw::c_ulong;

    static ORIG_TERMIOS: Mutex<Option<Vec<u8>>> = Mutex::new(None);

    // The termios struct size varies by platform. We use an oversized buffer.
    const TERMIOS_BUF_SIZE: usize = 256;

    // c_lflag offsets and bit masks
    #[cfg(target_os = "linux")]
    mod consts {
        use std::os::raw::c_ulong;
        pub const LFLAG_OFFSET: usize = 12; // c_lflag is at byte 12 on Linux
        pub const IFLAG_OFFSET: usize = 0;
        pub const OFLAG_OFFSET: usize = 4;
        pub const VMIN_OFFSET: usize = 22; // c_cc[VMIN] on Linux (VMIN=6, offset 17+6-1=22)
        pub const VTIME_OFFSET: usize = 23;
        pub const ECHO: u32 = 0o10;
        pub const ICANON: u32 = 0o2;
        pub const ISIG: u32 = 0o1;
        pub const IEXTEN: u32 = 0o100000;
        pub const IXON: u32 = 0o2000;
        pub const ICRNL: u32 = 0o400;
        pub const BRKINT: u32 = 0o2;
        pub const INPCK: u32 = 0o20;
        pub const ISTRIP: u32 = 0o40;
        pub const OPOST: u32 = 0o1;
        pub const TIOCGWINSZ: c_ulong = 0x5413;
    }

    #[cfg(target_os = "macos")]
    mod consts {
        use std::os::raw::c_ulong;
        pub const LFLAG_OFFSET: usize = 12;
        pub const IFLAG_OFFSET: usize = 0;
        pub const OFLAG_OFFSET: usize = 4;
        pub const VMIN_OFFSET: usize = 32; // c_cc starts at byte 20 on macOS, VMIN=16 → 20+16=36... varies
        pub const VTIME_OFFSET: usize = 33;
        pub const ECHO: u32 = 0x8;
        pub const ICANON: u32 = 0x100;
        pub const ISIG: u32 = 0x80;
        pub const IEXTEN: u32 = 0x400;
        pub const IXON: u32 = 0x200;
        pub const ICRNL: u32 = 0x100;
        pub const BRKINT: u32 = 0x2;
        pub const INPCK: u32 = 0x10;
        pub const ISTRIP: u32 = 0x20;
        pub const OPOST: u32 = 0x1;
        pub const TIOCGWINSZ: c_ulong = 0x40087468;
    }

    // Fallback for other unix
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    mod consts {
        pub const LFLAG_OFFSET: usize = 12;
        pub const IFLAG_OFFSET: usize = 0;
        pub const OFLAG_OFFSET: usize = 4;
        pub const VMIN_OFFSET: usize = 22;
        pub const VTIME_OFFSET: usize = 23;
        pub const ECHO: u32 = 0o10;
        pub const ICANON: u32 = 0o2;
        pub const ISIG: u32 = 0o1;
        pub const IEXTEN: u32 = 0o100000;
        pub const IXON: u32 = 0o2000;
        pub const ICRNL: u32 = 0o400;
        pub const BRKINT: u32 = 0o2;
        pub const INPCK: u32 = 0o20;
        pub const ISTRIP: u32 = 0o40;
        pub const OPOST: u32 = 0o1;
        pub const TIOCGWINSZ: c_ulong = 0x5413;
    }

    extern "C" {
        fn tcgetattr(fd: i32, termios: *mut u8) -> i32;
        fn tcsetattr(fd: i32, action: i32, termios: *const u8) -> i32;
        fn ioctl(fd: i32, request: c_ulong, ...) -> i32;
    }

    const TCSANOW: i32 = 0;

    fn read_u32(buf: &[u8], offset: usize) -> u32 {
        u32::from_ne_bytes([buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]])
    }

    fn write_u32(buf: &mut [u8], offset: usize, val: u32) {
        let bytes = val.to_ne_bytes();
        buf[offset..offset + 4].copy_from_slice(&bytes);
    }

    pub fn enable_raw_mode() {
        unsafe {
            let mut buf = vec![0u8; TERMIOS_BUF_SIZE];
            if tcgetattr(0, buf.as_mut_ptr()) != 0 {
                return;
            }
            *ORIG_TERMIOS.lock().unwrap() = Some(buf.clone());

            // Modify c_lflag
            let lflag = read_u32(&buf, consts::LFLAG_OFFSET);
            let new_lflag = lflag & !(consts::ECHO | consts::ICANON | consts::ISIG | consts::IEXTEN);
            write_u32(&mut buf, consts::LFLAG_OFFSET, new_lflag);

            // Modify c_iflag
            let iflag = read_u32(&buf, consts::IFLAG_OFFSET);
            let new_iflag = iflag & !(consts::IXON | consts::ICRNL | consts::BRKINT | consts::INPCK | consts::ISTRIP);
            write_u32(&mut buf, consts::IFLAG_OFFSET, new_iflag);

            // Keep OPOST in c_oflag
            let oflag = read_u32(&buf, consts::OFLAG_OFFSET);
            write_u32(&mut buf, consts::OFLAG_OFFSET, oflag | consts::OPOST);

            // Set VMIN=0, VTIME=1 (100ms timeout)
            buf[consts::VMIN_OFFSET] = 0;
            buf[consts::VTIME_OFFSET] = 1;

            tcsetattr(0, TCSANOW, buf.as_ptr());
        }
    }

    pub fn disable_raw_mode() {
        unsafe {
            if let Some(ref buf) = *ORIG_TERMIOS.lock().unwrap() {
                tcsetattr(0, TCSANOW, buf.as_ptr());
            }
        }
    }

    #[repr(C)]
    struct Winsize {
        ws_row: u16,
        ws_col: u16,
        ws_xpixel: u16,
        ws_ypixel: u16,
    }

    pub fn terminal_size() -> (u16, u16) {
        unsafe {
            let mut ws: Winsize = std::mem::zeroed();
            if ioctl(1, consts::TIOCGWINSZ, &mut ws as *mut Winsize) == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
                (ws.ws_col, ws.ws_row)
            } else {
                (80, 24)
            }
        }
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

pub fn enable_raw_mode() {
    platform::enable_raw_mode();
}

pub fn disable_raw_mode() {
    platform::disable_raw_mode();
}

pub fn enter_alt_screen() {
    print!("{}{}", ALT_SCREEN_ON, CURSOR_HIDE);
    let _ = io::stdout().flush();
}

pub fn leave_alt_screen() {
    print!("{}{}", CURSOR_SHOW, ALT_SCREEN_OFF);
    let _ = io::stdout().flush();
}

pub fn terminal_size() -> (u16, u16) {
    platform::terminal_size()
}

#[allow(dead_code)]
pub fn clear_and_home() {
    print!("{}{}", CLEAR_SCREEN, CURSOR_HOME);
    let _ = io::stdout().flush();
}

/// Flicker-free repaint: move cursor home, overwrite content in place,
/// clear remainder of each line and everything below the last line.
/// Emits a single write to stdout to minimise tearing.
pub fn paint(output: &str) {
    // Pre-allocate: original size + headroom for escape sequences
    let mut buf = String::with_capacity(output.len() + 512);
    buf.push_str(CURSOR_HOME);
    for line in output.split('\n') {
        buf.push_str(line);
        buf.push_str(CLEAR_TO_EOL);
        buf.push('\n');
    }
    buf.push_str(CLEAR_TO_EOS);
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = handle.write_all(buf.as_bytes());
    let _ = handle.flush();
}

pub fn read_key() -> Option<Key> {
    let mut buf = [0u8; 1];
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    match handle.read(&mut buf) {
        Ok(0) => return None,
        Err(_) => return None,
        Ok(_) => {}
    }

    match buf[0] {
        0x03 => Some(Key::CtrlC),
        0x0D => Some(Key::Enter),
        0x0A => {
            // On Unix raw mode with ICRNL cleared, Enter produces 0x0D.
            // But just in case:
            Some(Key::Enter)
        }
        0x1B => {
            // Escape sequence
            let mut seq = [0u8; 1];
            match handle.read(&mut seq) {
                Ok(0) | Err(_) => return Some(Key::Escape),
                Ok(_) => {}
            }
            if seq[0] == b'[' {
                let mut code = [0u8; 1];
                match handle.read(&mut code) {
                    Ok(0) | Err(_) => return Some(Key::Escape),
                    Ok(_) => {}
                }
                match code[0] {
                    b'A' => Some(Key::Up),
                    b'B' => Some(Key::Down),
                    b'C' => Some(Key::Right),
                    b'D' => Some(Key::Left),
                    b'H' => Some(Key::Home),
                    b'F' => Some(Key::End),
                    b'1' | b'4' | b'7' | b'8' => {
                        // Could be 1~ (Home), 4~ (End), etc.
                        let mut tilde = [0u8; 1];
                        let _ = handle.read(&mut tilde);
                        match code[0] {
                            b'1' | b'7' => Some(Key::Home),
                            b'4' | b'8' => Some(Key::End),
                            _ => Some(Key::Unknown),
                        }
                    }
                    _ => Some(Key::Unknown),
                }
            } else {
                Some(Key::Escape)
            }
        }
        0x20 => Some(Key::Space),
        b => {
            if b.is_ascii_graphic() {
                Some(Key::Char(b as char))
            } else {
                Some(Key::Unknown)
            }
        }
    }
}
