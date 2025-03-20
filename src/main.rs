use dirs::config_dir;
use ini::Ini;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::process::Command;

fn convert_gnome_to_ghossty_shortcut(
    gnome_shortcuts: HashMap<String, String>,
) -> HashMap<String, String> {
    let gnome_to_ghostty = HashMap::from([
        ("Primary", "ctrl"), // check os maybe
        ("Shift", "shift"),
        ("Alt", "alt"),
        ("equal", "="),
        ("Left", "l"),
        ("Down", "d"),
        ("Up", "u"),
        ("Right", "r"),
    ]);

    gnome_shortcuts
        .iter()
        .map(|(command, binding)| {
            let replaced = binding
                .split(&['>', '<'][..])
                .filter(|c| !c.is_empty())
                .map(|gnome_shortcut| {
                    gnome_to_ghostty
                        .get(gnome_shortcut)
                        .copied()
                        .unwrap_or(gnome_shortcut)
                })
                .fold(String::new(), |acc, s| acc + "+" + &s);
            (command.to_string(), replaced[1..].to_string())
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
    let y = convert_gnome_to_ghossty_shortcut(x);
    println!("{:?}", y);
    let z = get_ghossty_shortcuts(None);
    println! {"{:?}", z};
}
