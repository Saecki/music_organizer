use std::path::PathBuf;
use std::process::exit;
use walkdir::WalkDir;
use std::ffi::{OsStr, OsString};
use clap::{App, Arg, Shell};
use std::io::Write;
use std::str::FromStr;

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
    pub title: String,
    pub current_file: PathBuf,
}

#[derive(Default, Debug, PartialEq)]
pub struct Metadata {
    pub track: u16,
    pub artist: String,
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
                let artist = match tag.album_artist() {
                    Some(s) => s.to_string(),
                    None => tag.artist().unwrap_or("").to_string(),
                };

                return Self {
                    track,
                    artist,
                    title: tag.title().unwrap_or("").to_string(),
                    album: tag.album().unwrap_or("").to_string(),
                };
            } else {},
            "m4a" | "m4b" | "m4p" | "m4v" => if let Ok(tag) = mp4ameta::Tag::read_from_path(&path) {
                let track = match tag.track_number() {
                    Some((t, _)) => t as u16,
                    None => 0,
                };
                let artist = match tag.album_artist() {
                    Some(s) => s.to_string(),
                    None => tag.artist().unwrap_or("").to_string(),
                };

                return Self {
                    track,
                    artist,
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
        app.clone().gen_completions("playlist_localizer", shell, output_dir);
        println!("done");
        exit(0);
    }

    let music_dir = PathBuf::from(matches.value_of("music-dir").unwrap());
    let copy = matches.is_present("copy");
    let yes = matches.is_present("assume-yes");

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
        .filter(|e| match e.metadata() {
            Ok(m) => m.is_file(),
            Err(_e) => false,
        })
    {
        let p = d.into_path();
        if !is_music_extension(p.extension().unwrap()) { continue; }

        let m = Metadata::read_from(&p);
        let song_index = songs.len();
        songs.push(Song {
            track: m.track,
            title: m.title,
            current_file: p,
        });

        overwrite_last_line(&format!("\rsong {}", song_index + 1));
        let _ = std::io::stdout().flush().is_ok();

        if m.artist.is_empty() {
            unknown.push(song_index);
            continue;
        }

        if artists.is_empty() {
            artists.push(Artist {
                name: m.artist,
                albums: vec![Album {
                    name: m.album,
                    songs: vec![song_index],
                }],
            });

            continue;
        }

        for ar in &mut artists {
            if ar.name == m.artist {
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
            name: m.artist,
            albums: vec![Album {
                name: m.album,
                songs: vec![song_index],
            }],
        });
    }

    if !yes {
        loop {
            print!(
                "\n{} files will be {}. Continue [y/N]?",
                songs.len(),
                if copy { "copied" } else { "moved" }
            );
            let _ = std::io::stdout().flush().is_ok();

            let mut input = String::with_capacity(2);
            if let Err(e) = std::io::stdin().read_line(&mut input) {
                println!("error: {}", e);
            } else if input.to_lowercase() == "y\n" {
                break;
            } else if input.to_lowercase() == "n\n" {
                println!("exiting...");
                exit(1);
            }
        }
    }

    unsafe {
        LAST_LEN = 0;
    }

    println!("\nwriting...");
    let mut counter: usize = 1;
    for ar in &artists {
        let ar_os_str = valid_os_string(&ar.name);
        let ar_dir = output_dir.clone().join(&ar_os_str);
        if !ar_dir.exists() {
            if let Err(e) = std::fs::create_dir(&ar_dir) {
                println!("error creating dir: {}:\n{}", ar_dir.display(), e);
            }
        }

        for al in &ar.albums {
            let al_os_str = valid_os_string(&al.name);
            let al_dir = ar_dir.clone().join(&al_os_str);
            if !al_dir.exists() {
                if let Err(e) = std::fs::create_dir(&al_dir) {
                    println!("error creating dir: {}:\n{}", al_dir.display(), e);
                }
            }

            for si in &al.songs {
                let song = &songs[*si];
                let extension = song.current_file.extension().unwrap();

                if al.name.is_empty() {
                    let mut file_name = OsString::with_capacity(4 + ar_os_str.len() + song.title.len() + extension.len());

                    file_name.push(&ar_os_str);
                    file_name.push(" - ");
                    file_name.push(valid_os_string(&song.title));
                    file_name.push(".");
                    file_name.push(extension);

                    let new_file = ar_dir.join(file_name);

                    mv_or_cp(&counter, &song.current_file, &new_file, copy);
                } else {
                    let mut file_name = OsString::with_capacity(9 + ar_os_str.len() + song.title.len() + extension.len());

                    file_name.push(format!("{:02} - ", song.track));
                    file_name.push(&ar_os_str);
                    file_name.push(" - ");
                    file_name.push(valid_os_string(&song.title));
                    file_name.push(".");
                    file_name.push(extension);

                    let new_file = al_dir.join(file_name);

                    mv_or_cp(&counter, &song.current_file, &new_file, copy);
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

            mv_or_cp(&counter, &song.current_file, &new_file, copy);
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

fn mv_or_cp(song_index: &usize, old: &PathBuf, new: &PathBuf, copy: bool) {
    if copy {
        overwrite_last_line(&format!("copying {} {}", song_index, new.display()));
        let _ = std::io::stdout().flush().is_ok();
        if let Err(e) = std::fs::copy(old, new) {
            println!("\nerror: {}", e);
        }
    } else {
        overwrite_last_line(&format!("moving {} {}", song_index, new.display()));
        let _ = std::io::stdout().flush().is_ok();
        if let Err(e) = std::fs::rename(old, new) {
            println!("\nerror: {}", e);
        }
    }
}

#[inline]
fn overwrite_last_line(str: &str) {
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
