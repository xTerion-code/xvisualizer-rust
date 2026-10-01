use crate::theme::Theme;

pub struct Args {
    pub device: Option<String>,
    pub no_color: bool,
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
    parse_from(std::env::args().skip(1))
}

fn parse_from(it: impl Iterator<Item = String>) -> Args {
    let mut device: Option<String> = None;
    // Honor https://no-color.org/ in addition to the flag.
    let mut no_color = std::env::var_os("NO_COLOR").is_some();
    let mut theme: Option<Theme> = None;
    let mut it = it.peekable();
    while let Some(a) = it.next() {
        // Support both `--flag value` and `--flag=value`.
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (a.clone(), None),
        };
        match flag.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-d" | "--device" => {
                let v = inline.or_else(|| it.next()).unwrap_or_else(|| {
                    eprintln!("error: {flag} requires a device name");
                    std::process::exit(2);
                });
                device = Some(v);
            }
            "-t" | "--theme" => {
                let v = inline.or_else(|| it.next()).unwrap_or_else(|| {
                    eprintln!("error: {flag} requires classic|solid");
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
