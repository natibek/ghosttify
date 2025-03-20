use dirs::config_dir;
use ini::Ini;
use serde_json::from_str;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::process::Command;

static MAP_STRING: &str = include_str!("./gnome_to_ghostty.json");

fn convert_gnome_to_ghossty_shortcut(
    gnome_shortcuts: HashMap<String, String>,
) -> HashMap<String, String> {
    let gnome_to_ghostty: HashMap<String, HashMap<String, String>> = from_str(MAP_STRING).unwrap();
    let gnome_to_ghostty_shortcut = gnome_to_ghostty.get("keys").unwrap();
    let gnome_to_ghostty_action = gnome_to_ghostty.get("actions").unwrap();

    gnome_shortcuts
        .iter()
        .flat_map(|(command, binding)| {
            let ghostty_command = gnome_to_ghostty_action.get(command);
            match ghostty_command {
                Some(com) => {
                    if com.is_empty() {
                        return None;
                    }
                }
                None => return None,
            }
            let ghostty_binding = binding
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

            if ghostty_binding.is_empty() {
                return None;
            }

            Some((
                ghostty_command.unwrap().to_string(),
                ghostty_binding[1..].to_string(),
            ))
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
        .map(|(command, binding)| (command.to_string(), binding.to_string()))
        .collect::<HashMap<String, String>>()
}

fn get_ghossty_shortcuts(ghostty_config: Option<&str>) -> HashMap<String, String> {
    let config_dir = config_dir().unwrap();
    let mut config_file = config_dir.join("ghostty").join("config");

    if let Some(f) = ghostty_config {
        config_file = PathBuf::from(f);
    }

    if !config_file.exists() {
        println!("File not found");
    }
    let file = File::open(config_file).unwrap();
    io::BufReader::new(file)
        .lines()
        .filter_map(Result::ok)
        .filter(|line| line.starts_with("keybind = "))
        .map(|line| {
            let split = line
                .trim_start_matches("keybind = ")
                .split_once('=')
                .unwrap();

            (split.1.to_string(), split.0.to_string())
        })
        .collect::<HashMap<String, String>>()
}

fn main() {
    let x = get_gnome_shortcuts();
    println!("{:?}", x);
    let y = convert_gnome_to_ghossty_shortcut(x);
    println!("{:?}", y);
    get_ghossty_shortcuts(None);
}
