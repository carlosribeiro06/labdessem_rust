//! Terminal banner and display utilities for the labdessem CLI.

use std::io::{stderr, IsTerminal};

/// Check if colored output should be used.
/// Returns true when stderr is connected to a TTY.
pub fn use_colors() -> bool {
    stderr().is_terminal()
}

/// Print the labdessem banner to stderr.
pub fn print_banner() {
    let colored = use_colors();
    let banner = render_banner(colored);
    eprint!("{banner}");
}

fn render_banner(use_color: bool) -> String {
    let version = env!("CARGO_PKG_VERSION");
    if use_color {
        format!(
            "\n\x1b[1;38;5;39m  ⚡ LABDESSEM\x1b[0m v{version}\n     \
             \x1b[38;5;245mHydrothermal dispatch optimization\x1b[0m\n\n"
        )
    } else {
        format!(
            "\n  ⚡ LABDESSEM v{version}\n     \
             Hydrothermal dispatch optimization\n\n"
        )
    }
}

/// Execution configuration for display.
pub struct ExecutionInfo<'a> {
    pub case_path: &'a str,
    pub execution_mode: &'a str,
    pub network_enabled: bool,
    pub uct_enabled: bool,
    pub uch_enabled: bool,
    pub fpha_enabled: bool,
    pub fcf_enabled: bool,
    pub ton_residual_enabled: bool,
}

/// Print execution configuration to stderr.
pub fn print_execution_info(info: &ExecutionInfo) {
    let colored = use_colors();
    let output = render_execution_info(info, colored);
    eprint!("{output}");
}

fn render_execution_info(info: &ExecutionInfo, use_color: bool) -> String {
    let mut flags = Vec::new();
    if info.uct_enabled {
        flags.push("UCT");
    }
    if info.uch_enabled {
        flags.push("UCH");
    }
    if info.fpha_enabled {
        flags.push("FPHA");
    }
    if info.fcf_enabled {
        flags.push("FCF");
    }
    if info.ton_residual_enabled {
        flags.push("TON_Residual");
    }
    if info.network_enabled {
        flags.push("rede");
    }

    let flags_str = if flags.is_empty() {
        "none".to_string()
    } else {
        flags.join(", ")
    };

    if use_color {
        format!(
            "\x1b[1mExecution\x1b[0m\n  \
             \x1b[38;5;245mCase:\x1b[0m      {}\n  \
             \x1b[38;5;245mSolver:\x1b[0m    HiGHS\n  \
             \x1b[38;5;245mMode:\x1b[0m      {}\n  \
             \x1b[38;5;245mFlags:\x1b[0m     {}\n\n",
            info.case_path, info.execution_mode, flags_str
        )
    } else {
        format!(
            "Execution\n  \
             Case:      {}\n  \
             Solver:    HiGHS\n  \
             Mode:      {}\n  \
             Flags:     {}\n\n",
            info.case_path, info.execution_mode, flags_str
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{render_banner, render_execution_info, ExecutionInfo};

    #[test]
    fn render_banner_plain_contains_version() {
        let banner = render_banner(false);
        assert!(banner.contains("LABDESSEM v"));
        assert!(!banner.contains("\x1b["));
    }

    #[test]
    fn render_banner_colored_contains_ansi() {
        let banner = render_banner(true);
        assert!(banner.contains("\x1b["));
        assert!(banner.contains("\x1b[0m"));
    }

    #[test]
    fn render_banner_contains_tagline() {
        let banner = render_banner(false);
        assert!(banner.contains("Hydrothermal dispatch optimization"));
    }

    #[test]
    fn render_banner_contains_lightning() {
        let banner = render_banner(false);
        assert!(banner.contains("⚡"));
    }

    #[test]
    fn render_execution_info_shows_enabled_flags() {
        let info = ExecutionInfo {
            case_path: "/test/path",
            execution_mode: "MILP único",
            network_enabled: false,
            uct_enabled: true,
            uch_enabled: false,
            fpha_enabled: false,
            fcf_enabled: true,
            ton_residual_enabled: false,
        };
        let output = render_execution_info(&info, false);
        assert!(output.contains("UCT"));
        assert!(output.contains("FCF"));
        assert!(!output.contains("UCH"));
        assert!(!output.contains("FPHA"));
        assert!(!output.contains("rede"));
    }

    #[test]
    fn render_execution_info_shows_none_when_no_flags() {
        let info = ExecutionInfo {
            case_path: "/test/path",
            execution_mode: "MILP único",
            network_enabled: false,
            uct_enabled: false,
            uch_enabled: false,
            fpha_enabled: false,
            fcf_enabled: false,
            ton_residual_enabled: false,
        };
        let output = render_execution_info(&info, false);
        assert!(output.contains("Flags:     none"));
    }

    #[test]
    fn render_execution_info_colored_contains_ansi() {
        let info = ExecutionInfo {
            case_path: "/test/path",
            execution_mode: "MILP único",
            network_enabled: false,
            uct_enabled: true,
            uch_enabled: false,
            fpha_enabled: false,
            fcf_enabled: false,
            ton_residual_enabled: false,
        };
        let output = render_execution_info(&info, true);
        assert!(output.contains("\x1b[1m"));
        assert!(output.contains("\x1b[0m"));
    }

    #[test]
    fn render_execution_info_shows_case_and_mode() {
        let info = ExecutionInfo {
            case_path: "/my/case",
            execution_mode: "LP iterativo",
            network_enabled: false,
            uct_enabled: false,
            uch_enabled: false,
            fpha_enabled: false,
            fcf_enabled: false,
            ton_residual_enabled: false,
        };
        let output = render_execution_info(&info, false);
        assert!(output.contains("/my/case"));
        assert!(output.contains("LP iterativo"));
        assert!(output.contains("HiGHS"));
    }
}

