use colored::Colorize;
use dirs::config_dir;
use ini::Ini;
use regex::Regex;
use serde_json::from_str;
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Add gnome terminal shortcuts to ghostty config in the config-file "gnome-shortcuts".
    #[arg(short, long, default_value_t = false)]
    apply: bool,
    
    /// If shortcut conflicts with an existing keybinding, don't add to the new config.
    #[arg(short = 'c', long, default_value_t = false)]
    avoid_conflict: bool,

    /// Print found non-default ghostty keybindings.
    #[arg(long, default_value_t = false)]
    ghostty: bool,

    /// Print gnome shortcuts.
    #[arg(long, default_value_t = false)]
    gnome: bool,
}

static MAP_STRING: &str = include_str!("./gnome_to_ghostty.json");

/// Convert a gnome shortcut to ghostty using the `gnome_to_ghostty.json` file which provides
/// mappings for the repsentation of keys used in gnome configurations to ghostty's. gnome
/// shortcuts surround special keys with angle brackets and don't use any delimiting character
/// between keys in a shortcut. When converting,
///     - if no ghostty key is found for the gnome key in the mapping, use the same gnome key,
///     - if the gnome shortcut is `disabled`, ignore the gnome shortcut,
///     - if the ghostty key is an empty string, ignore the gnome shortcut (not a supported key).
/// https://github.com/ghostty-org/ghostty/blob/d6e76858164d52cff460fedc61ddf2e560912d71/src/input/key.zig#L255
///
/// Args:
/// - gnome_shortcut: The gnome shorcut being converted.
/// - gnome_to_ghostty_shortcut: A hashmap with a mapping from gnome configuration key
///     representatioin to ghostty's.
fn convert_gnome_shortcut_to_ghostty(
    gnome_shortcut: &String,
    gnome_to_ghostty_shortcut: &HashMap<String, String>,
) -> Option<String> {
    let ghostty_shortcut = gnome_shortcut
        .split(&['>', '<'][..])
        .filter(|key| !key.is_empty())
        .filter_map(|key| {
            let mapped = gnome_to_ghostty_shortcut.get(key);

            match mapped {
                Some(ghostty_key) => {
                    if ghostty_key.is_empty() || ghostty_key == "disabled" {
                        return None;
                    } else {
                        return Some(ghostty_key.to_string());
                    }
                }
                None => Some(key.to_string()),
            }
        })
        .fold(String::new(), |acc, s| acc + "+" + &s);

    if ghostty_shortcut.is_empty() {
        return None;
    }
    Some(ghostty_shortcut[1..].to_string())
}

/// Converts all the gnome shortcuts to ghostty shortcuts using the
/// `convert_gnome_shortcut_to_ghostty` function. If the shortcut can not be converted or the
/// action has no parallel in ghostty, the shortcut is ignore. The conversions for both the keys
/// and the actions are in the gnome_to_ghostty.json` file.
///
/// Args:
/// - gnome_shortcuts: A hashmap of the gnome shorcuts with the action as the key and shortcut as
///     the value.
fn convert_gnome_to_ghostty_shortcuts(
    gnome_shortcuts: HashMap<String, String>,
) -> HashMap<String, String> {
    let gnome_to_ghostty: HashMap<String, HashMap<String, String>> = from_str(MAP_STRING).unwrap();
    let gnome_to_ghostty_shortcut = gnome_to_ghostty.get("keys").unwrap();
    let gnome_to_ghostty_action = gnome_to_ghostty.get("actions").unwrap();

    gnome_shortcuts
        .iter()
        .flat_map(|(action, binding)| {
            let ghostty_action = gnome_to_ghostty_action.get(action);
            match ghostty_action {
                Some(com) => {
                    if com.is_empty() {
                        return None;
                    }
                }
                None => return None,
            }

            let ghostty_shortcut = if let Some(shortcut) =
                convert_gnome_shortcut_to_ghostty(binding, &gnome_to_ghostty_shortcut)
            {
                shortcut
            } else {
                return None;
            };

            Some((ghostty_action.unwrap().to_string(), ghostty_shortcut))
        })
        .collect()
}

/// Get the gnome shortcuts using `dconf dump /org/gnome/terminal/ and produce a map with an
/// action key and shortcut value.
fn get_gnome_shortcuts() -> HashMap<String, String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg("dconf dump /org/gnome/terminal/")
        .output()
        .expect("Error running `dconf dump /org/gnome/terminal/`. Make sure `dconf` installation is valid");

    let conf = String::from_utf8(output.stdout).unwrap();
    let ini_conf = Ini::load_from_str(&conf).unwrap();
    let keybindings = ini_conf.section(Some("legacy/keybindings")).unwrap();

    keybindings
        .into_iter()
        .map(|(action, binding)| (action.to_string(), binding.to_string()))
        .collect::<HashMap<String, String>>()
}

/// Gets all the ghostty config files including the main `config` file. The rest are
/// files provided through the "config-file" option. Using BFS to preserve order logic enforced
/// by ghostty configuration.
fn get_config_files() -> Vec<String> {
    // also check the optional config files with the ? before the quoted name
    let config_file_re = Regex::new(r#"config-file\s*=\s*(?:"([^"]+)"|([^"]+))\s*"#).unwrap();
    let ghostty_config_dir = config_dir().unwrap().join("ghostty");
    let mut config_file_paths = vec!["config".to_string()];
    let mut stack = VecDeque::from(["config".to_string()]);

    while !stack.is_empty() {
        let cur_config_dir = stack.pop_front().unwrap();

        if let Ok(lines) = read_lines(ghostty_config_dir.join(cur_config_dir)) {
            for line in lines.map_while(Result::ok) {
                if let Some(cap) = config_file_re.captures(&line) {
                    // the name of the config could be quoted or not
                    let config_file = cap.get(1).map_or_else(|| &cap[2], |m| m.as_str());
                    if Path::new(&ghostty_config_dir.join(config_file)).exists() {
                        config_file_paths.push(config_file.to_string());
                        stack.push_back(config_file.to_string());
                    }
                }
            }
        }
    }

    config_file_paths
}

/// Get all the shortcuts defined in the ghostty config. This includes keybindings in config files
/// provided by the "config-file" option. https://ghostty.org/docs/config/reference#config-file
/// Also, bindings defined later shadow earlier ones if in the same file.
fn get_ghostty_shortcuts() -> HashMap<String, String> {
    let ghostty_config_dir = config_dir().unwrap().join("ghostty");
    let keybinding_re = Regex::new(r#"keybind\s*=\s*(.*)=\s*(.*)\s*"#).unwrap();

    let mut shortcuts: HashMap<String, String> = HashMap::new();
    let config_file_paths = get_config_files();

    for config_file_path in config_file_paths {
        let config_file = ghostty_config_dir.join(config_file_path);

        if let Ok(lines) = read_lines(config_file) {
            for line in lines.map_while(Result::ok) {
                if let Some(cap) = keybinding_re.captures(&line) {
                    shortcuts.insert(cap[2].to_string(), cap[1].to_string());
                }
            }
        }
    }

    shortcuts
}
/// Print the ghostty shortcuts.
///
/// Args:
/// - shortcuts: map from the action to the keybinding
fn print_ghostty_shortcuts(shortcuts: &HashMap<String, String>) {
    println!(
        "\t    {}\t{}",
        "Binding".to_string().cyan(),
        "Action".to_string().magenta()
    );
    for (action, binding) in shortcuts {
        println!("keybind = {}={}", binding.bright_cyan(), action.magenta());
    }
    println!();
}

/// Updates the ghostty config with converted gnome shortcuts. New bindings are added if
/// the action does not only have the same binding already.
///
/// Args:
/// - converted_gnome_shortcuts: map from the action to the keybinding of the converted gnome
///     shortcuts
/// - ghostty_shortcuts: map from the action to the keybinding for the ghostty config shortcuts.
///    This accounts for different config files and the order in which bindings are stated.
/// - avoid_conflict: only apply shortcuts that do not conflict with existing key bindings
///
fn update_ghostty_config(
    converted_gnome_shortcuts: HashMap<String, String>,
    ghostty_shortcuts: HashMap<String, String>,
    avoid_conflict: bool,
) {
    let re = Regex::new(r#"config-file\s*=\s*gnome-shortcuts"#).unwrap();
    let ghostty_config_dir = config_dir().unwrap().join("ghostty");
    let ghostty_config = ghostty_config_dir.join("config");

    let mut config_file = OpenOptions::new()
        .append(true)
        .read(true)
        .open(ghostty_config)
        .expect("File Error");

    let config_found: bool = io::BufReader::new(&config_file)
        .lines()
        .filter_map(Result::ok)
        .any(|line| re.is_match(&line));

    let gnome_shortcuts_path = ghostty_config_dir.join("gnome-shortcuts");

    let mut gnome_shortcuts_config: File = if !config_found {
        config_file.write_all(b"\n# Added by ghosttify\n").unwrap();
        config_file
            .write_all(b"config-file=gnome-shortcuts\n")
            .unwrap();
        File::create(gnome_shortcuts_path).expect("File Error")
    } else {
        OpenOptions::new()
            .append(true)
            .read(true)
            .open(gnome_shortcuts_path)
            .expect("File Error")
    };

    let keybindings: HashMap<&String, &String> = if avoid_conflict {
        ghostty_shortcuts.iter().map(|(key, value)| (value, key) ).collect()
    } else {
        HashMap::new()
    }; 

    for (action, binding) in &converted_gnome_shortcuts {
        if (avoid_conflict && (!ghostty_shortcuts.contains_key(action) && !keybindings.contains_key(binding))) || !avoid_conflict {
            gnome_shortcuts_config
                .write_all(format!("keybind = {}={}\n", binding, action).as_bytes())
                .unwrap();
        }
    }
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn main() {
    let args = Cli::parse();

    let gnome_shortcuts = get_gnome_shortcuts();
    let converted_shortcuts = convert_gnome_to_ghostty_shortcuts(gnome_shortcuts);
    if args.gnome {
        println!("{}", "Gnome Shortcuts".italic().bold().bright_blue());
        print_ghostty_shortcuts(&converted_shortcuts);
    }

    let ghostty_shortcuts = get_ghostty_shortcuts();
    if args.ghostty {
        println!("{}", "Ghostty Shortcuts".italic().bold().bright_blue());
        print_ghostty_shortcuts(&ghostty_shortcuts);
    }

    if args.apply {
        update_ghostty_config(converted_shortcuts, ghostty_shortcuts, args.avoid_conflict);
       
    }
}
