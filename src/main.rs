use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;

use clap::{App, Arg, Shell};
use walkdir::WalkDir;

const MUSIC_FILE_EXTENSIONS: [&str; 5] = [
    "m4a",
    "mp3",
    "m4b",
    "m4p",
    "m4v",
];

static mut LAST_LEN: usize = 0;

#[derive(Debug, PartialEq)]
pub struct Artist {
    pub name: String,
    pub albums: Vec<Album>,
}

#[derive(Debug, PartialEq)]
pub struct Album {
    pub name: String,
    pub songs: Vec<usize>,
}

#[derive(Default, Debug, PartialEq)]
pub struct Song {
    pub track: u16,
    pub artist: String,
    pub title: String,
    pub current_file: PathBuf,
}

#[derive(Default, Debug, PartialEq)]
pub struct Metadata {
    pub track: u16,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub title: String,
}

impl Metadata {
    pub fn read_from(path: &PathBuf) -> Self {
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => if let Ok(tag) = id3::Tag::read_from_path(&path) {
                let track = match tag.track() {
                    Some(t) => t as u16,
                    None => 0,
                };

                return Self {
                    track,
                    artist: tag.artist().unwrap_or("").to_string(),
                    album_artist: tag.album_artist().unwrap_or("").to_string(),
                    title: tag.title().unwrap_or("").to_string(),
                    album: tag.album().unwrap_or("").to_string(),
                };
            } else {},
            "m4a" | "m4b" | "m4p" | "m4v" => if let Ok(tag) = mp4ameta::Tag::read_from_path(&path) {
                let track = match tag.track_number() {
                    Some((t, _)) => t as u16,
                    None => 0,
                };

                return Self {
                    track,
                    artist: tag.artist().unwrap_or("").to_string(),
                    album_artist: tag.album_artist().unwrap_or("").to_string(),
                    title: tag.title().unwrap_or("").to_string(),
                    album: tag.album().unwrap_or("").to_string(),
                };
            },
            _ => (),
        }

        Self::default()
    }
}

fn main() {
    let app = App::new("music organizer")
        .version("0.1.0")
        .author("Saecki")
        .about("Moves or copies and renames Music files using their metadata information.")
        .arg(Arg::with_name("music-dir")
            .short("m")
            .long("music-dir")
            .help("The directory which will be searched for music files")
            .takes_value(true)
            .required_unless("generate-completion")
            .conflicts_with("generate-completion"))
        .arg(Arg::with_name("output-dir")
            .short("o")
            .long("output-dir")
            .help("The directory which the content will be written to")
            .takes_value(true))
        .arg(Arg::with_name("copy")
            .short("c")
            .long("copy")
            .help("Copy the files instead of moving")
            .requires("output-dir")
            .takes_value(false))
        .arg(Arg::with_name("assume-yes")
            .short("y")
            .long("assume-yes")
            .help("Assumes yes as a answer for all questions")
            .takes_value(false))
        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .help("Verbose output")
            .takes_value(false))
        .arg(Arg::with_name("generate-completion")
            .short("g")
            .long("generate-completion")
            .value_name("shell")
            .help("Generates a completion script for the specified shell")
            .conflicts_with("music-dir")
            .requires("output-dir")
            .takes_value(true)
            .possible_values(&["bash", "zsh", "fish", "elvish", "powershell"])
        );

    let matches = app.clone().get_matches();
    let generate_completion = matches.value_of("generate-completion").unwrap_or("");


    if generate_completion != "" {
        let output_dir = PathBuf::from(matches.value_of("output-dir").unwrap());
        if !output_dir.exists() {
            match std::fs::create_dir_all(&output_dir) {
                Ok(_) => println!("created dir: {}", output_dir.display()),
                Err(e) => println!("error creating dir: {}\n{}", output_dir.display(), e),
            }
        }

        println!("generating completions...");
        let shell = Shell::from_str(generate_completion).unwrap();
        app.clone().gen_completions("music_organizer", shell, output_dir);
        println!("done");
        exit(0);
    }

    let music_dir = PathBuf::from(matches.value_of("music-dir").unwrap());
    let copy = matches.is_present("copy");
    let yes = matches.is_present("assume-yes");
    let verbose = matches.is_present("verbose");

    let output_dir = match matches.value_of("output-dir") {
        Some(s) => PathBuf::from(s),
        None => music_dir.clone(),
    };

    if !output_dir.exists() {
        match std::fs::create_dir_all(&output_dir) {
            Ok(_) => println!("created dir: {}", output_dir.display()),
            Err(e) => println!("error creating dir: {}\n{}", output_dir.display(), e),
        }
    }

    let abs_music_dir = match PathBuf::from(&music_dir).canonicalize() {
        Ok(t) => t,
        Err(e) => {
            println!("Not a valid music dir path: {}\n{:?}", music_dir.display(), e);
            exit(1)
        }
    };

    println!("indexing...");
    let mut artists = Vec::new();
    let mut unknown = Vec::new();
    let mut songs = Vec::new();

    'songs: for d in WalkDir::new(&abs_music_dir).into_iter()
        .filter_entry(|e| !e.file_name()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
        )
        .filter_map(|e| e.ok())
        .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
    {
        let p = d.into_path();
        if !is_music_extension(p.extension().unwrap()) { continue; }

        let m = Metadata::read_from(&p);
        let song_index = songs.len();

        print_verbose(&format!("{} {} - {}", song_index + 1, &m.artist, &m.title), verbose);

        songs.push(Song {
            track: m.track,
            artist: m.artist.clone(),
            title: m.title,
            current_file: p,
        });

        let _ = std::io::stdout().flush().is_ok();

        let artist = if !m.album_artist.is_empty() {
            m.album_artist
        } else if !m.artist.is_empty() {
            m.artist
        } else {
            unknown.push(song_index);
            continue;
        };

        if artists.is_empty() {
            artists.push(Artist {
                name: artist,
                albums: vec![Album {
                    name: m.album,
                    songs: vec![song_index],
                }],
            });

            continue;
        }

        for ar in &mut artists {
            if ar.name == artist {
                for al in &mut ar.albums {
                    if al.name == m.album {
                        al.songs.push(song_index);
                        continue 'songs;
                    }
                }

                ar.albums.push(Album {
                    name: m.album,
                    songs: vec![song_index],
                });
                continue 'songs;
            }
        }

        artists.push(Artist {
            name: artist,
            albums: vec![Album {
                name: m.album,
                songs: vec![song_index],
            }],
        });
    }

    println!("\nchecking songs");

    for (i, ar1) in artists.iter().enumerate() {
        for (j, ar2) in artists.iter().enumerate() {
            if i != j && ar1.name.eq_ignore_ascii_case(&ar2.name) {
                println!("These two artists are named similarly:\n{}\n{}", &ar1.name, &ar2.name);
                let index = input_options_loop(&[
                    "don't do anything",
                    "merge using first",
                    "merge using second",
                    "enter new name"
                ]);

                match index {
                    0 => continue,
                    1 => println!("update first"),
                    2 => println!("update second"),
                    3 => loop {
                        let new_name = input_loop("enter new name:", |_| true);
                        println!("new name: '{}'", new_name);

                        let index = input_options_loop(&[
                            "ok",
                            "reenter name",
                            "dismiss",
                        ]);

                        match index {
                            0 => println!("rename"),
                            1 => continue,
                            _ => break,
                        }
                    }
                    _ => continue,
                }
            }
        }
    }

    if !yes {
        let ok = input_confirmation_loop(&format!(
            "{} files will be {}. Continue",
            songs.len(),
            if copy { "copied" } else { "moved" })
        );

        if !ok {
            println!("exiting...");
            exit(1);
        }
    }

    unsafe {
        LAST_LEN = 0;
    }

    println!("\nwriting...");
    let mut counter: usize = 1;
    for ar in &artists {
        let ar_dir = output_dir.clone().join(valid_os_string(&ar.name));
        if !ar_dir.exists() {
            if let Err(e) = std::fs::create_dir(&ar_dir) {
                println!("error creating dir: {}:\n{}", ar_dir.display(), e);
            }
        }

        for al in &ar.albums {
            let al_dir = ar_dir.clone().join(valid_os_string(&al.name));
            if !al_dir.exists() {
                if let Err(e) = std::fs::create_dir(&al_dir) {
                    println!("error creating dir: {}:\n{}", al_dir.display(), e);
                }
            }

            for si in &al.songs {
                let song = &songs[*si];
                let extension = song.current_file.extension().unwrap();

                if al.name.is_empty() || al.name.to_ascii_lowercase() == format!("{} - single", &song.title.to_ascii_lowercase()) {
                    let mut file_name = OsString::with_capacity(4 + song.artist.len() + song.title.len() + extension.len());

                    file_name.push(valid_os_string(&song.artist));
                    file_name.push(" - ");
                    file_name.push(valid_os_string(&song.title));
                    file_name.push(".");
                    file_name.push(extension);

                    let new_file = ar_dir.join(file_name);

                    mv_or_cp(&counter, &song.current_file, &new_file, copy, verbose);
                } else {
                    let mut file_name = OsString::with_capacity(9 + song.artist.len() + song.title.len() + extension.len());

                    file_name.push(format!("{:02} - ", song.track));
                    file_name.push(valid_os_string(&song.artist));
                    file_name.push(" - ");
                    file_name.push(valid_os_string(&song.title));
                    file_name.push(".");
                    file_name.push(extension);

                    let new_file = al_dir.join(file_name);

                    mv_or_cp(&counter, &song.current_file, &new_file, copy, verbose);
                }
                counter += 1;
            }
        }
    }

    if !unknown.is_empty() {
        let unknown_dir = output_dir.join("unknown");
        if !unknown_dir.exists() {
            if let Err(e) = std::fs::create_dir(&unknown_dir) {
                println!("Error creating dir: {}:\n{}", unknown_dir.display(), e);
            }
        }
        for si in &unknown {
            let song = &songs[*si];
            let new_file = unknown_dir.join(song.current_file.file_name().unwrap());

            mv_or_cp(&counter, &song.current_file, &new_file, copy, verbose);
            counter += 1;
        }
        println!();
    }

    println!("\ndone")
}

#[inline]
fn is_music_extension(s: &OsStr) -> bool {
    for e in &MUSIC_FILE_EXTENSIONS {
        if s.eq(*e) {
            return true;
        }
    }

    false
}

fn mv_or_cp(song_index: &usize, old: &PathBuf, new: &PathBuf, copy: bool, verbose: bool) {
    if old == new {
        print_verbose(&format!("skipping {} {}", song_index, new.display()), verbose);
    } else if copy {
        print_verbose(&format!("copying {} {}", song_index, new.display()), verbose);
        let _ = std::io::stdout().flush().is_ok();
        if let Err(e) = std::fs::copy(old, new) {
            println!("\nerror: {}", e);
        }
    } else {
        print_verbose(&format!("moving {} {}", song_index, new.display()), verbose);
        let _ = std::io::stdout().flush().is_ok();
        if let Err(e) = std::fs::rename(old, new) {
            println!("\nerror: {}", e);
        }
    }
}

fn input_loop(str: &str, predicate: fn(&str) -> bool) -> String {
    let mut input = String::with_capacity(10);

    loop {
        println!("{}", str);

        match std::io::stdin().read_line(&mut input) {
            Ok(_) => if predicate(&input) { return input; },
            Err(e) => println!("error:\n {}", e),
        }
    }
}

fn input_options_loop(options: &[&str]) -> usize {
    let mut input = String::with_capacity(2);

    loop {
        for (i, s) in options.iter().enumerate() {
            println!("[{}] {}", i, s);
        }

        match std::io::stdin().read_line(&mut input) {
            Ok(_) => match usize::from_str(input.trim_matches('\n')) {
                Ok(i) => if i < options.len() {
                    return i;
                } else {
                    println!("invalid input")
                },
                Err(_) => println!("invalid input"),
            }
            Err(e) => println!("error:\n {}", e),
        }
    }
}

fn input_confirmation_loop(str: &str) -> bool {
    let mut input = String::with_capacity(2);

    loop {
        print!("{} [y/N]?", str);
        let _ = std::io::stdout().flush().is_ok();

        if let Err(e) = std::io::stdin().read_line(&mut input) {
            println!("error:\n {}", e);
        } else {
            input.make_ascii_lowercase();

            if input == "\n" || input == "y\n" {
                return true;
            } else if input == "n\n" {
                return false;
            } else {
                println!("invalid input");
            }
        }
    }
}

#[inline]
fn print_verbose(str: &str, verbose: bool) {
    if verbose {
        println!("{}", str);
    } else {
        let len = str.chars().count();
        let diff = unsafe { LAST_LEN as i32 - len as i32 };

        print!("\r{}", str);
        for _ in 0..diff {
            print!(" ");
        }
        let _ = std::io::stdout().flush().is_ok();

        unsafe {
            LAST_LEN = len;
        }
    }
}

lazy_static::lazy_static! {
    static ref RE: regex::Regex = regex::Regex::new(r#"[<>:"/\|?*]"#).unwrap();
}

fn valid_os_string(str: &str) -> OsString {
    let mut s = RE.replace_all(str, "").to_string();

    if s.starts_with('.') {
        s.replace_range(0..1, "_")
    }

    if s.ends_with('.') {
        s.replace_range(s.len() - 1..s.len(), "_")
    }

    OsString::from(s)
}
