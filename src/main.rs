use colored::Colorize;
use dirs::config_dir;
use ini::Ini;
use regex::Regex;
use serde_json::from_str;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

static MAP_STRING: &str = include_str!("./gnome_to_ghostty.json");

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

fn get_ghostty_shortcuts_for_config_file(
    config_file_name: &str,
    current_shortcuts: &mut HashMap<String, String>,
    config_file_re: &Regex,
    keybinding_re: &Regex,
) {
    let config_dir = config_dir().unwrap();
    let config_file = config_dir.join("ghostty").join(config_file_name);

    if let Ok(lines) = read_lines(config_file) {
        for line in lines.map_while(Result::ok) {
            if let Some(cap) = &config_file_re.captures(&line) {
                get_ghostty_shortcuts_for_config_file(
                    &cap[1],
                    current_shortcuts,
                    &config_file_re,
                    &keybinding_re,
                );
            } else if let Some(cap) = keybinding_re.captures(&line) {
                current_shortcuts.insert(cap[2].to_string(), cap[1].to_string());
            }
        }
    }
}

fn get_ghostty_shortcuts() -> HashMap<String, String> {
    let config_file_re = Regex::new(r#"config-file\s*=\s*(.*)\s*"#).unwrap();
    let keybinding_re = Regex::new(r#"keybind\s*=\s*(.*)=\s*(.*)\s*"#).unwrap();
    let mut shortcuts: HashMap<String, String> = HashMap::new();
    get_ghostty_shortcuts_for_config_file(
        "config",
        &mut shortcuts,
        &config_file_re,
        &keybinding_re,
    );

    shortcuts
}

fn print_ghostty_config(converted_gnome_shortcuts: &HashMap<String, String>) {
    println!(
        "\t    {}\t{}",
        "Binding".to_string().cyan(),
        "Action".to_string().magenta()
    );
    for (action, binding) in converted_gnome_shortcuts {
        println!("keybind = {}={}", binding.bright_cyan(), action.magenta());
    }
    println!();
}

fn update_ghostty_config(
    converted_gnome_shortcuts: HashMap<String, String>,
    ghostty_shortcuts: HashMap<String, String>,
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

    for (action, binding) in &converted_gnome_shortcuts {
        if !ghostty_shortcuts.contains_key(action) {
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
    let gnome_shortcuts = get_gnome_shortcuts();
    print_ghostty_config(&gnome_shortcuts);
    let converted_shortcuts = convert_gnome_to_ghostty_shortcuts(gnome_shortcuts);
    print_ghostty_config(&converted_shortcuts);
    let ghostty_shortcuts = get_ghostty_shortcuts();
    print_ghostty_config(&ghostty_shortcuts);

    update_ghostty_config(converted_shortcuts, ghostty_shortcuts);
}
