use std::path::Path;
use std::process::Command;
use wait_timeout::ChildExt;

#[derive(Debug, Default, Clone)]
enum Status {
    #[default]
    Undefined,
    Stopped,
    Playing(Info),
    Paused(Info),
}

#[derive(Debug, Default, Clone)]
struct Info {
    file: String,
    duration: u32,
    position: u32,
    artist: Option<String>,
    album: Option<String>,
    title: Option<String>,
    date: Option<u32>,
    track_number: Option<u32>,
}

fn parse(lines: &[&str]) -> Status {
    let mut s = Status::Undefined;
    let mut info = Info::default();

    for line in lines {
        let line: Vec<_> = line.split_whitespace().collect();
        if line[0] == "status" {
            match line[1] {
                "stopped" => s = Status::Stopped,
                "paused" => s = Status::Paused(info.clone()),
                "playing" => s = Status::Playing(info.clone()),
                _ => unimplemented!(),
            }
        } else if line[0] == "file" {
            info.file = line[1..].join(" ").to_string();
        } else if line[0] == "duration" {
            info.duration = line[1].parse().expect("Could not parse to integer");
        } else if line[0] == "position" {
            info.position = line[1].parse().expect("Could not parse to integer");
        } else if line[0] == "tag" {
            if line[1] == "artist" {
                info.artist = Some(line[2..].join(" ").to_string());
            } else if line[1] == "album" {
                info.album = Some(line[2..].join(" ").to_string());
            } else if line[1] == "title" {
                info.title = Some(line[2..].join(" ").to_string());
            } else if line[1] == "date" {
                info.date = Some(line[2].parse().expect("Could not parse to integer"));
            } else if line[1] == "tracknumber" {
                info.track_number = Some(line[2].parse().expect("Could not parse to integer"));
            }
        }
    }

    match s {
        Status::Stopped => Status::Stopped,
        Status::Paused(_) => Status::Paused(info),
        Status::Playing(_) => Status::Playing(info),
        Status::Undefined => Status::Undefined,
    }
}

fn action(prev_lyrics: Option<String>) -> Option<String> {
    let c = Command::new("cmus-remote")
        .arg("-Q")
        .output()
        .expect("No output captured");

    let output = std::str::from_utf8(c.stdout.as_slice())
        .unwrap()
        .to_string();

    let lines: Vec<_> = output.lines().collect();

    let status = parse(&lines);

    match status {
        Status::Undefined | Status::Stopped => prev_lyrics,
        Status::Playing(info) | Status::Paused(info) => {
            let lyrics_file = (info.file + ".lyrics").to_string();

            if let Some(p_lyrics) = prev_lyrics {
                if p_lyrics == lyrics_file {
                    return Some(p_lyrics.clone());
                }
            }

            match Path::new(&lyrics_file).exists() {
                true => {
                    let mut child = Command::new("bat")
                        .arg(lyrics_file.clone())
                        .arg(&format!(
                            "--file-name={}-{}",
                            info.title.unwrap_or("Unknown Title".to_string()),
                            info.album.unwrap_or("Unknown Album".to_string()),
                        ))
                        .spawn()
                        .unwrap();

                    let three_secs = Duration::from_secs(3);
                    match child.wait_timeout(three_secs).unwrap() {
                        Some(_) => {}
                        None => {
                            child.kill().unwrap();
                        }
                    };
                    Some(lyrics_file)
                }
                false => {
                    let mut c = Command::new("lyrics")
                        .spawn()
                        .expect("Lyrics utility didn't work");

                    c.wait().expect("Lyrics utility failed to spawn");
                    Some(lyrics_file)
                }
            }
        }
    }
}

use std::{thread, time::Duration};

fn main() {
    let wait_time = Duration::from_secs(1);

    let mut prev_lyrics_file = None;

    loop {
        let new_lyrics_file = action(prev_lyrics_file.clone());
        prev_lyrics_file = new_lyrics_file;
        thread::sleep(wait_time);
    }
}
