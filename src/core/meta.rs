use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleaseArtists<'a> {
    pub names: &'a [String],
    pub releases: Vec<Release<'a>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Release<'a> {
    pub name: &'a str,
    pub songs: Vec<&'a Song>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
    pub path: PathBuf,
    pub track_number: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc_number: Option<u16>,
    pub total_discs: Option<u16>,
    pub release_artists: Vec<String>,
    pub artists: Vec<String>,
    pub release: String,
    pub title: String,
    pub has_artwork: bool,
}

impl Song {
    pub fn artists_str(&self) -> String {
        self.artists.join(", ")
    }

    pub fn release_artists_str(&self) -> String {
        self.release_artists.join(", ")
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub track_number: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc_number: Option<u16>,
    pub total_discs: Option<u16>,
    pub artists: Vec<String>,
    pub release_artists: Vec<String>,
    pub release: Option<String>,
    pub title: Option<String>,
    pub has_artwork: bool,
}

impl Metadata {
    pub fn read_from(path: &Path) -> Self {
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => {
                if let Some(meta) = Self::read_mp3(path) {
                    return meta;
                }
            }
            "m4a" => {
                if let Some(meta) = Self::read_mp4(path) {
                    return meta;
                }
            }
            _ => (),
        }

        Self::default()
    }

    fn read_mp3(path: &Path) -> Option<Self> {
        let tag = id3::Tag::read_from_path(&path).ok()?;
        let m = Self {
            track_number: zero_none(tag.track().map(|u| u as u16)),
            total_tracks: zero_none(tag.total_tracks().map(|u| u as u16)),
            disc_number: zero_none(tag.disc().map(|u| u as u16)),
            total_discs: zero_none(tag.total_discs().map(|u| u as u16)),
            artists: tag
                .artist()
                .map(|s| s.split('\u{0}').map(|s| s.to_string()).collect())
                .unwrap_or(Vec::new()),
            release_artists: tag
                .album_artist()
                .map(|s| s.split('\u{0}').map(|s| s.to_string()).collect())
                .unwrap_or(Vec::new()),
            release: tag.album().map(|s| s.to_string()),
            title: tag.title().map(|s| s.to_string()),
            has_artwork: tag.pictures().next().is_some(),
        };

        Some(m)
    }

    fn read_mp4(path: &Path) -> Option<Self> {
        let mut tag = mp4ameta::Tag::read_from_path(&path).ok()?;
        let m = Self {
            track_number: tag.track_number(),
            total_tracks: tag.total_tracks(),
            disc_number: tag.disc_number(),
            total_discs: tag.total_discs(),
            artists: tag.take_artists().collect(),
            release_artists: tag.take_album_artists().collect(),
            release: tag.take_album(),
            title: tag.take_title(),
            has_artwork: tag.artwork().is_some(),
        };

        Some(m)
    }

    pub fn release_artists(&self) -> Option<&[String]> {
        if !self.release_artists.is_empty() {
            Some(&self.release_artists)
        } else if !self.artists.is_empty() {
            Some(&self.artists)
        } else {
            None
        }
    }

    pub fn song_artists(&self) -> Option<&[String]> {
        if !self.artists.is_empty() {
            Some(&self.artists)
        } else if !self.release_artists.is_empty() {
            Some(&self.release_artists)
        } else {
            None
        }
    }
}

#[inline]
pub fn zero_none(n: Option<u16>) -> Option<u16> {
    n.and_then(|n| match n {
        0 => None,
        _ => Some(n),
    })
}
