//! Worker threads report back through the event loop; a panic must still report, or the app waits forever.

/// Runs `f` if the thread panics while this is alive (dropped while unwinding); otherwise does nothing.
pub struct OnPanic<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> OnPanic<F> {
    pub fn new(f: F) -> Self {
        Self(Some(f))
    }
}

impl<F: FnOnce()> Drop for OnPanic<F> {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(f) = self.0.take()
        {
            f();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    // Review Focus 4.
    #[test]
    fn the_guard_runs_when_the_thread_panics() {
        let (tx, rx) = channel();
        let tx2 = tx.clone();
        let _ = std::thread::spawn(move || {
            let _guard = OnPanic::new(move || tx2.send("panicked").unwrap());
            panic!("boom");
        })
        .join();
        assert_eq!(rx.try_recv(), Ok("panicked"));
        std::thread::spawn(move || {
            let _guard = OnPanic::new(move || tx.send("panicked").unwrap());
        })
        .join()
        .unwrap();
        assert!(rx.try_recv().is_err(), "a normal end sends nothing");
    }
}
