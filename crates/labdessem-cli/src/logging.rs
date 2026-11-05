//! Log output of the labdessem CLI: `tracing` events, and `log` records bridged from
//! dependencies such as HiGHS, go to stderr at WARN unless `RUST_LOG` says otherwise.

use std::env;
use std::error::Error;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

/// Event filter: WARN when `directives` is absent or empty, else the `RUST_LOG` directives.
/// Invalid directives are reported on stderr and ignored.
fn log_filter(directives: Option<&str>) -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .parse_lossy(directives.unwrap_or_default())
}

/// Installs the global subscriber: one plain line per event on stderr, without
/// timestamp, target or colour.
///
/// # Errors
///
/// Returns an error when a global `tracing` subscriber or `log` logger is already set.
pub fn init() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(log_filter(env::var("RUST_LOG").ok().as_deref()))
        .without_time()
        .with_target(false)
        .try_init()
}

#[cfg(test)]
mod tests {
    use super::{init, log_filter};
    use tracing_subscriber::filter::LevelFilter;

    #[test]
    fn log_filter_defaults_to_warn_and_honours_directives() {
        assert_eq!(log_filter(None).max_level_hint(), Some(LevelFilter::WARN));
        assert_eq!(
            log_filter(Some("")).max_level_hint(),
            Some(LevelFilter::WARN)
        );
        assert_eq!(
            log_filter(Some("debug")).max_level_hint(),
            Some(LevelFilter::DEBUG)
        );
        assert_eq!(
            log_filter(Some("labdessem_simulation=info")).max_level_hint(),
            Some(LevelFilter::INFO)
        );
        // An unparsable level is ignored, so the WARN default applies.
        assert_eq!(
            log_filter(Some("labdessem=loud")).max_level_hint(),
            Some(LevelFilter::WARN)
        );
    }

    #[test]
    fn init_installs_the_global_subscriber_once() {
        assert!(init().is_ok());
        assert!(init().is_err());
    }
}
