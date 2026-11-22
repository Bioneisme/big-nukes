//! The mod's log lines. The launcher builds a native mod as its own library against a game already built, so the
//! mod has a `log` of its own that no one gave a logger: what it logs would be lost. This one writes to the game's
//! standard error, as the game's own lines are, in the game's form, so the launcher's output shows them. Where the
//! game's logger is the mod's too (a game built with the mod), the game's stays and this one is never set.

use std::io::Write;

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, meta: &log::Metadata) -> bool {
        meta.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let thread = std::thread::current();
        let line = format!("[{}/{}]: {}\n", thread.name().unwrap_or("unnamed"), record.level(), record.args());
        let _ = std::io::stderr().lock().write_all(line.as_bytes());
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

static LOGGER: Logger = Logger;

/// Gives the mod's `log` its logger, once; nothing if it has one.
pub(crate) fn init() {
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}
