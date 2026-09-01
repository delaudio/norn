use std::{
    io::{self, Stdout},
    panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Once, OnceLock,
    },
};

use crossterm::{
    cursor::{Hide, Show},
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    style::Print,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};

pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    interrupted: Arc<AtomicBool>,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        install_panic_restore_hook();
        let interrupted = install_sigint_restore_handler()?;

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste,
            Hide
        )?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            interrupted,
        })
    }

    pub fn draw<F>(&mut self, draw: F) -> io::Result<()>
    where
        F: FnOnce(&mut ratatui::Frame<'_>),
    {
        self.terminal.draw(draw)?;
        Ok(())
    }

    pub fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    pub fn area(&self) -> io::Result<Rect> {
        self.terminal
            .size()
            .map(|size| Rect::new(0, 0, size.width, size.height))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
}

fn install_panic_restore_hook() {
    static PANIC_HOOK: Once = Once::new();
    PANIC_HOOK.call_once(|| {
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let _ = restore_terminal();
            original_hook(panic_info);
        }));
    });
}

fn install_sigint_restore_handler() -> io::Result<Arc<AtomicBool>> {
    static INTERRUPTED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    let interrupted = INTERRUPTED
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone();

    static SIGINT_HANDLER: Once = Once::new();
    let handler_flag = interrupted.clone();
    let mut install_result = Ok(());
    SIGINT_HANDLER.call_once(|| {
        install_result = ctrlc::set_handler(move || {
            handler_flag.store(true, Ordering::SeqCst);
            let _ = restore_terminal();
        })
        .map_err(io::Error::other);
    });
    install_result?;
    interrupted.store(false, Ordering::SeqCst);
    Ok(interrupted)
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        io::stdout(),
        Show,
        DisableBracketedPaste,
        LeaveAlternateScreen,
        DisableMouseCapture
    )
}

pub fn copy_to_clipboard(text: &str) -> io::Result<()> {
    execute!(io::stdout(), Print(osc52_sequence(text)))?;
    Ok(())
}

fn osc52_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64_encode(text.as_bytes()))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let packed = ((first as u32) << 16) | ((second as u32) << 8) | third as u32;

        output.push(TABLE[((packed >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((packed >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[((packed >> 6) & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(packed & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{base64_encode, osc52_sequence};

    #[test]
    fn encodes_clipboard_payload_for_osc52() {
        assert_eq!(base64_encode(b"AI review"), "QUkgcmV2aWV3");
        assert_eq!(osc52_sequence("ok"), "\x1b]52;c;b2s=\x07");
    }
}
