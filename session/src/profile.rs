//! Work profiles: open the tools for a kind of work in one go.
//!
//! `--profile studio` opens the music tools, `art` the photo and painting tools,
//! `dev` the development tools. Tools that aren't installed are reported, with the
//! package to install, instead of failing silently. `--run "cmd args"` adds any
//! other program. The first program opened becomes the main (left) window.

use std::path::Path;

use crate::audio::{AudioMode, STUDIO, parse_latency};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// Shown to the user.
    pub label: &'static str,
    /// Program and arguments. No shell is involved.
    pub argv: Vec<String>,
    /// Debian/Ubuntu package that provides it.
    pub package: &'static str,
}

fn app(label: &'static str, command: &str, package: &'static str) -> App {
    App {
        label,
        argv: split_command(command).expect("built-in commands are well formed"),
        package,
    }
}

/// Split a command line into arguments, shell-style but without a shell:
/// whitespace separates, '...' is literal, "..." allows \" and \\, and a
/// backslash outside quotes escapes the next character. Nothing is expanded.
pub fn split_command(command: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_arg = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_arg = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => current.push(c),
                        None => return Err(format!("unclosed ' in {command:?}")),
                    }
                }
            }
            '"' => {
                in_arg = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => current.push(c),
                            Some(c) => {
                                current.push('\\');
                                current.push(c);
                            }
                            None => return Err(format!("unclosed \" in {command:?}")),
                        },
                        Some(c) => current.push(c),
                        None => return Err(format!("unclosed \" in {command:?}")),
                    }
                }
            }
            '\\' => {
                in_arg = true;
                match chars.next() {
                    Some(c) => current.push(c),
                    None => return Err(format!("trailing \\ in {command:?}")),
                }
            }
            c if c.is_whitespace() => {
                if in_arg {
                    args.push(std::mem::take(&mut current));
                    in_arg = false;
                }
            }
            c => {
                in_arg = true;
                current.push(c);
            }
        }
    }
    if in_arg {
        args.push(current);
    }
    Ok(args)
}

/// Built-in profiles. Main tool first.
pub fn profile(name: &str) -> Option<Vec<App>> {
    Some(match name {
        "studio" => vec![
            app("Ardour (DAW)", "ardour", "ardour"),
            app("qpwgraph (audio routing)", "qpwgraph", "qpwgraph"),
            app("Terminal", "foot", "foot"),
        ],
        "art" => vec![
            app("Krita (painting)", "krita", "krita"),
            app("darktable (photo)", "darktable", "darktable"),
            app("GIMP (image editing)", "gimp", "gimp"),
        ],
        "dev" => vec![
            app(
                "VS Code",
                "code --ozone-platform=wayland --enable-features=WaylandWindowDecorations",
                "code",
            ),
            app("Terminal", "foot", "foot"),
        ],
        _ => return None,
    })
}

pub const PROFILES: &[&str] = &["studio", "art", "dev"];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub apps: Vec<App>,
    pub errors: Vec<String>,
    /// Profiles asked for, in order.
    pub profiles: Vec<String>,
    /// `--latency`, if given.
    pub latency: Option<AudioMode>,
}

impl Plan {
    /// Audio for this session: `--latency` if given, low latency for the studio
    /// profile, otherwise PipeWire's own defaults.
    pub fn audio(&self) -> AudioMode {
        self.latency
            .unwrap_or(if self.profiles.iter().any(|p| p == "studio") {
                AudioMode::Fixed(STUDIO)
            } else {
                AudioMode::Default
            })
    }
}

/// Parse `--profile NAME` and `--run "CMD"` (both repeatable, in order) and
/// `--latency low|default|FRAMES[@RATE]`.
pub fn parse_args(args: &[String]) -> Plan {
    let mut plan = Plan::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--profile" | "-p" => match iter.next() {
                Some(name) => match profile(name) {
                    Some(apps) => {
                        plan.apps.extend(apps);
                        plan.profiles.push(name.clone());
                    }
                    None => plan.errors.push(format!(
                        "unknown profile {name:?} (choose from: {})",
                        PROFILES.join(", ")
                    )),
                },
                None => plan.errors.push("--profile needs a name".into()),
            },
            // `-c` / `--command` kept for compatibility with smallvil.
            "--run" | "-r" | "-c" | "--command" => match iter.next() {
                Some(command) if !command.trim().is_empty() => match split_command(command) {
                    Ok(argv) => plan.apps.push(App {
                        label: "custom",
                        argv,
                        package: "",
                    }),
                    Err(error) => plan.errors.push(error),
                },
                _ => plan.errors.push(format!("{arg} needs a command")),
            },
            "--latency" => match iter.next().map(|value| parse_latency(value)) {
                Some(Ok(mode)) => plan.latency = Some(mode),
                Some(Err(error)) => plan.errors.push(error),
                None => plan.errors.push("--latency needs a value".into()),
            },
            other => plan.errors.push(format!("unknown argument {other:?}")),
        }
    }
    plan
}

/// Whether `program` can be found: an explicit path that exists, or a name on PATH.
pub fn is_installed(program: &str, path_var: Option<&str>) -> bool {
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    path_var
        .unwrap_or("")
        .split(':')
        .filter(|dir| !dir.is_empty())
        .any(|dir| Path::new(dir).join(program).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn profiles_put_the_main_tool_first() {
        assert_eq!(profile("studio").unwrap()[0].argv, ["ardour"]);
        assert_eq!(profile("art").unwrap()[0].argv, ["krita"]);
        assert_eq!(profile("dev").unwrap()[0].argv[0], "code");
        assert!(profile("games").is_none());
    }

    #[test]
    fn parses_profiles_and_runs_in_order() {
        let plan = parse_args(&args(&["--run", "foot -e htop", "--profile", "dev"]));
        assert!(plan.errors.is_empty());
        assert_eq!(plan.apps[0].argv, ["foot", "-e", "htop"]);
        assert_eq!(plan.apps[1].label, "VS Code");
    }

    #[test]
    fn studio_gets_low_latency_unless_told_otherwise() {
        assert_eq!(
            parse_args(&args(&["-p", "studio"])).audio(),
            AudioMode::Fixed(STUDIO)
        );
        assert_eq!(
            parse_args(&args(&["-p", "art"])).audio(),
            AudioMode::Default
        );
        assert_eq!(
            parse_args(&args(&["-p", "studio", "--latency", "default"])).audio(),
            AudioMode::Default
        );
        assert_eq!(
            parse_args(&args(&["-p", "dev", "--latency", "low"])).audio(),
            AudioMode::Fixed(STUDIO)
        );
        assert_eq!(parse_args(&args(&["--latency", "100"])).errors.len(), 1);
    }

    #[test]
    fn reports_bad_arguments() {
        let plan = parse_args(&args(&["--profile", "games", "--run", "  ", "--wat"]));
        assert_eq!(plan.errors.len(), 3);
        assert!(plan.errors[0].contains("studio, art, dev"));
    }

    #[test]
    fn splits_commands_like_a_shell_without_running_one() {
        assert_eq!(
            split_command(r#"foot -e sh -c 'echo hi; uname -a'"#).unwrap(),
            ["foot", "-e", "sh", "-c", "echo hi; uname -a"]
        );
        assert_eq!(
            split_command(r#"a "b \"c\" d" e\ f ''"#).unwrap(),
            ["a", r#"b "c" d"#, "e f", ""]
        );
        assert_eq!(split_command("  x   $HOME  ").unwrap(), ["x", "$HOME"]);
        assert!(split_command("echo 'open").is_err());
        assert!(split_command("echo \"open").is_err());
        assert!(split_command("trailing\\").is_err());
    }

    #[test]
    fn finds_programs_on_path() {
        assert!(is_installed("sh", Some("/usr/bin:/bin")));
        assert!(!is_installed(
            "definitely-not-a-program-xyz",
            Some("/usr/bin:/bin")
        ));
        assert!(!is_installed("sh", None));
        assert!(is_installed("/bin/sh", None));
    }
}
