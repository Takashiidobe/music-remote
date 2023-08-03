use std::io::{Read, Seek, Write};
use std::process::{Child, Command, Stdio};
use std::{fs::File, path::Path};

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
            } else if line[1] == "date" || line[1] == "originaldate" {
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

fn write_to_tmp_file(
    prev_file: Option<File>,
    prev_child: Option<Child>,
    lyrics: String,
    info: Info,
) -> (File, Child) {
    let mut tmpfile = if let Some(f) = prev_file {
        f
    } else {
        File::create("/tmp/.tmp1").expect("File failed to create")
    };

    tmpfile.set_len(0).expect("Truncating file failed");
    tmpfile.rewind().expect("rewinding failed");

    let mut metadata_lines = 0;

    if let Some(title) = info.title {
        writeln!(tmpfile, "Title: {}", title).expect("writing to temp file failed");
        metadata_lines += 1;
    }

    if let Some(album) = info.album {
        writeln!(tmpfile, "Album: {}", album).expect("writing to temp file failed");
        metadata_lines += 1;
    }

    if let Some(artist) = info.artist {
        writeln!(tmpfile, "Artist: {}", artist).expect("writing to temp file failed");
        metadata_lines += 1;
    }

    if let Some(year) = info.date {
        writeln!(tmpfile, "Year: {}", year).expect("writing to temp file failed");
        metadata_lines += 1;
    }

    if let Some(track_num) = info.track_number {
        writeln!(tmpfile, "Track Number: {}", track_num).expect("writing to temp file failed");
        metadata_lines += 1;
    }

    if metadata_lines > 0 {
        writeln!(tmpfile).expect("Failed to write to file");
    }

    write!(tmpfile, "{}", lyrics).expect("Writing to temp file failed");

    tmpfile.rewind().expect("rewinding failed");

    let child = if let Some(c) = prev_child {
        c
    } else {
        Command::new("nvim")
            .arg("-R")
            .arg("/tmp/.tmp1")
            .spawn()
            .unwrap()
    };

    (tmpfile, child)
}

fn action(
    prev_lyrics: Option<String>,
    prev_file: Option<File>,
    prev_child: Option<Child>,
) -> (Option<String>, Option<File>, Option<Child>) {
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
        Status::Undefined | Status::Stopped => (prev_lyrics, prev_file, prev_child),
        Status::Playing(info) | Status::Paused(info) => {
            let lyrics_file = (info.file.clone() + ".lyrics").to_string();

            if let Some(p_lyrics) = prev_lyrics.clone() {
                if p_lyrics == lyrics_file {
                    return (Some(p_lyrics.clone()), prev_file, prev_child);
                }
            }

            match Path::new(&lyrics_file).exists() {
                true => {
                    let lyrics = std::fs::read_to_string(&lyrics_file).expect("reading failed");

                    let (tmpfile, child) = write_to_tmp_file(prev_file, prev_child, lyrics, info);

                    (Some(lyrics_file), Some(tmpfile), Some(child))
                }
                false => {
                    let mut s = String::default();
                    if let (Some(artist), Some(song)) = (info.artist.clone(), info.title.clone()) {
                        let mut c = Command::new("lyrics")
                            .args(["-t", &artist, &song])
                            .stdout(Stdio::piped())
                            .spawn()
                            .expect("Lyrics utility didn't work");

                        let stdout = c.stdout.as_mut().unwrap();

                        stdout.read_to_string(&mut s).unwrap();

                        c.wait().expect("lyrics failed to resolve");

                        let mut lyrics_file_handle =
                            File::create(&lyrics_file).expect("Failed to create lyrics file");

                        write!(lyrics_file_handle, "{}", s)
                            .expect("Failed to write lyrics to file");

                        let (tmpfile, child) = write_to_tmp_file(prev_file, prev_child, s, info);

                        (Some(lyrics_file), Some(tmpfile), Some(child))
                    } else {
                        (None, None, None)
                    }
                }
            }
        }
    }
}

use std::{thread, time::Duration};

fn main() {
    let wait_time = Duration::from_secs(1);

    let mut prev_lyrics_file = None;
    let mut prev_file = None;
    let mut prev_child = None;

    loop {
        let (new_lyrics_file, new_file, new_child) =
            action(prev_lyrics_file.clone(), prev_file, prev_child);
        prev_lyrics_file = new_lyrics_file;
        prev_file = new_file;
        prev_child = new_child;

        thread::sleep(wait_time);
    }
}
