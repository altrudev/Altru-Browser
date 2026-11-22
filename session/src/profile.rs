//! Work profiles: open the tools for a kind of work in one go.
//!
//! `--profile studio` opens the music tools, `art` the photo and painting tools,
//! `dev` the development tools. Tools that aren't installed are reported, with the
//! package to install, instead of failing silently. `--run "cmd args"` adds any
//! other program. The first program opened becomes the main (left) window.

use std::path::Path;

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
        argv: command.split_whitespace().map(str::to_string).collect(),
        package,
    }
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
}

/// Parse `--profile NAME` and `--run "CMD"` (both repeatable), in order.
pub fn parse_args(args: &[String]) -> Plan {
    let mut plan = Plan::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--profile" | "-p" => match iter.next() {
                Some(name) => match profile(name) {
                    Some(apps) => plan.apps.extend(apps),
                    None => plan.errors.push(format!(
                        "unknown profile {name:?} (choose from: {})",
                        PROFILES.join(", ")
                    )),
                },
                None => plan.errors.push("--profile needs a name".into()),
            },
            // `-c` / `--command` kept for compatibility with smallvil.
            "--run" | "-r" | "-c" | "--command" => match iter.next() {
                Some(command) if !command.trim().is_empty() => plan.apps.push(App {
                    label: "custom",
                    argv: command.split_whitespace().map(str::to_string).collect(),
                    package: "",
                }),
                _ => plan.errors.push(format!("{arg} needs a command")),
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
    fn reports_bad_arguments() {
        let plan = parse_args(&args(&["--profile", "games", "--run", "  ", "--wat"]));
        assert_eq!(plan.errors.len(), 3);
        assert!(plan.errors[0].contains("studio, art, dev"));
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
