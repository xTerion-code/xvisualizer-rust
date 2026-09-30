//! Command-line argument parsing.
//!
//! This module only deals with CLI flags. It performs no I/O
//! besides printing help / errors.

use crate::theme::Theme;

/// Parsed command-line options.
pub struct Args {
    /// PulseAudio source name. `None` means auto-detect.
    pub device: Option<String>,
    /// Disable ANSI colors.
    pub no_color: bool,
    /// Initial theme. `None` means the default theme.
    pub theme: Option<Theme>,
}

pub fn print_help() {
    println!("xvisualizer - ASCII visualizer for system audio");
    println!();
    println!("Usage: xvisualizer [OPTIONS]");
    println!();
    println!("Options:");
    println!(
        "  -d, --device NAME Pulse source (default: auto-detected monitor of the default sink)"
    );
    println!("  -t, --theme NAME  start theme: classic | solid (default: classic)");
    println!("      --no-color    disable ANSI colors");
    println!("  -h, --help        show this help");
    println!();
    println!("Keys (in program):");
    println!("  Up/Down or Left/Right  choose theme in the menu");
    println!("  Enter                  apply theme");
    println!("  T / Tab                open theme menu, Esc - back");
    println!("  S                      capture source: system mix or one app");
    println!("  R (in source menu)     refresh the app list");
    println!("  1 / 2                  quick theme switch");
    println!("  Q or Ctrl+C            quit");
}

pub fn parse_args() -> Args {
    let mut device: Option<String> = None;
    let mut no_color = false;
    let mut theme: Option<Theme> = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-d" | "--device" => {
                let v = it.next().unwrap_or_else(|| {
                    eprintln!("error: {a} requires a device name");
                    std::process::exit(2);
                });
                device = Some(v);
            }
            "-t" | "--theme" => {
                let v = it.next().unwrap_or_else(|| {
                    eprintln!("error: {a} requires classic|solid");
                    std::process::exit(2);
                });
                match Theme::from_str(&v) {
                    Some(t) => theme = Some(t),
                    None => {
                        eprintln!("error: unknown theme '{v}' (classic|solid)");
                        std::process::exit(2);
                    }
                }
            }
            "--no-color" => no_color = true,
            _ => {
                eprintln!("error: unknown flag '{a}'. See xvisualizer --help");
                std::process::exit(2);
            }
        }
    }
    Args {
        device,
        no_color,
        theme,
    }
}
