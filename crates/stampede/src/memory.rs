use std::time::Duration;

const TRIM_EVERY: Duration = Duration::from_secs(30);

/// glibc keeps what a burst frees, such as replaying a long session, so without trimming the server stays at its peak.
pub fn trim_periodically() {
    tokio::spawn(async {
        let mut every = tokio::time::interval(TRIM_EVERY);
        loop {
            every.tick().await;
            trim();
        }
    });
}

#[cfg(target_env = "gnu")]
fn trim() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    unsafe { malloc_trim(0) };
}

#[cfg(not(target_env = "gnu"))]
fn trim() {}
