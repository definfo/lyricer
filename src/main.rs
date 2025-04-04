use anyhow::Result;
use std::io::Write;

use error::FormatError;
use mpris::Player;

mod error;
mod lyric;

const ERROR_LOOP_DURATION: std::time::Duration = std::time::Duration::from_secs(1);

fn main() -> Result<()> {
    pretty_env_logger::init();
    log::warn!("Lyricer started.");

    'outer: loop {
        let player = match find_player() {
            Ok(i) => i,
            Err(_) => {
                log::warn!("Player not found.");
                std::thread::sleep(ERROR_LOOP_DURATION);
                continue 'outer;
            }
        };
        'inner: loop {
            let res = main_logic(&player);
            match res {
                Ok(_) => {}
                Err(e) => {
                    if let FormatError::PlayerStopped = e {
                        break 'inner;
                    }
                }
            }
        }
        // When there's no available player, remove possible files
        clean_up();
    }
}

fn main_logic(player: &Player) -> Result<(), FormatError> {
    // Check if player is running
    if !player.is_running() {
        log::warn!("Player has stopped. Tryning to find a new player..");
        return Err(FormatError::PlayerStopped);
    }

    // Get metadata
    let metadata = match player.get_metadata() {
        Ok(i) => i,
        Err(i) => {
            log::warn!("Error when fetching metadata: {}", i);
            return Err(FormatError::MetadataError(i));
        }
    };
    log::debug!("Metadata found: {:?}", metadata);

    // Parse metadata
    let audio_ending = metadata.length();
    log::debug!("Audio length: {:?}", audio_ending);
    let mut formatted_metadata = metadata.title().unwrap_or("[Unknown]").to_string();
    if let Some(i) = audio_ending {
        formatted_metadata.push_str(&format!(" ({:#?})", i));
    }
    if let Some(i) = metadata.artists() {
        if !i.is_empty() {
            formatted_metadata.push_str(&format!("by {}", i.join(", ")));
        }
    }
    log::info!("Formatted metadata: {}", formatted_metadata);

    // Get lyrics
    let audio_path_cow =
        match urlencoding::decode(match metadata.url().unwrap_or("/").split("://").last() {
            Some(i) => i,
            None => {
                log::warn!("Failed to parse audio path.");
                return Err(FormatError::AudioParseError);
            }
        }) {
            Ok(i) => i,
            Err(i) => {
                log::warn!("Failed to find audio path: {}", i);
                return Err(FormatError::AudioNotFoundError(Box::new(i)));
            }
        };
    log::info!("Audio path: {}", audio_path_cow);
    let audio_path_binding = audio_path_cow.into_owned();
    let audio_path = std::path::Path::new(&audio_path_binding);
    let lyrics = get_lyrics(audio_path);
    print_lyrics(
        std::path::Path::new("/tmp/lyrics"),
        lyrics,
        &formatted_metadata,
        audio_ending,
        player.get_position().ok(),
        player,
    );
    Ok(())
}

fn get_lyrics(audio_path: &std::path::Path) -> Result<lyric::Lyric, ()> {
    if audio_path.is_file() {
        let mut lyric_name = std::path::PathBuf::from(&audio_path);
        if lyric_name.is_file() {
            lyric_name.set_extension("lrc");
            if let Ok(i) = std::fs::read(lyric_name) {
                log::info!("Audio lyrics found. Parsing...");
                return lyric::Lyric::parse(String::from_utf8_lossy(&i).to_string());
            }
        }
    }
    Err(())
}
fn print_lyrics(
    target_file: &std::path::Path,
    lyrics: Result<lyric::Lyric, ()>,
    formatted_metadata: &str,
    audio_ending: Option<std::time::Duration>,
    current_offset: Option<std::time::Duration>,
    player_handle: &mpris::Player,
) {
    let mut current_audio = String::new();
    let mut is_current_audio = || {
        // If the audio has changed, then recall the function
        if let Ok(i) = player_handle.get_metadata() {
            let mut constructed = String::new();
            if let Some(i) = i.url() {
                constructed.push_str(i);
            }
            if let Some(i) = i.title() {
                constructed.push_str(i);
            }
            if constructed == current_audio {
                true
            } else {
                current_audio = constructed;
                false
            }
        } else {
            false
        }
    };
    is_current_audio();
    let small_duration = lyrics.is_err();
    let real_lyrics = match lyrics {
        Ok(i) => i.content,
        Err(_) => Box::new([lyric::LyricsType::Standard(
            std::time::Duration::default(),
            Box::from("No lyrics"),
        )]),
    };
    let mut current_duration = current_offset.unwrap_or_default();
    for i in real_lyrics.as_ref() {
        if !is_current_audio() {
            log::warn!("Current audio has changed.");
            return;
        }
        // We can't implement colored lyrics yet
        let (duration, lyric) = match i {
            lyric::LyricsType::Standard(i, j) => (i, j.as_ref().to_string()),
            lyric::LyricsType::Enhanced(i, j) => (
                i,
                j.as_ref()
                    .iter()
                    .map(|x| x.1.as_ref().to_string())
                    .collect::<Vec<String>>()
                    .join(" "),
            ),
        };
        if lyric.trim().is_empty() {
            continue;
        }
        if small_duration {
            std::thread::sleep(std::time::Duration::from_secs(1))
        } else if let Some(a) = duration.checked_sub(current_duration) {
            std::thread::sleep(a)
        }
        if !player_handle.is_running() {
            log::warn!("Player is not running. Stopping...");
            return;
        }
        //current_duration += duration.to_owned();
        if let Ok(i) = player_handle.get_position() {
            if i > (current_duration + std::time::Duration::from_millis(125)) {
                // Current playing position is 1 second faster than current display.
                log::warn!("Subtitle too slow. Trying to sync up...");
            } else if i + std::time::Duration::from_millis(125) < current_duration {
                // Current playing position is 1 seond slower than current display.
                // This code design disallow "return" to a point. Thus, we will simply request to recall the function.
                log::warn!("Subtitle too fast. Trying to sync up...");
                return;
            }
            current_duration = i
        } else {
            log::warn!("Player does not implement position command. Subtitle might not synced.");
            current_duration = duration.to_owned()
        }
        let mut file = std::fs::File::create(target_file).expect("Create failed");
        file.write_all(output(&lyric, formatted_metadata).to_string().as_bytes())
            .expect("Cannot write metadata into file");
    }
    if let Some(audio_ending) = audio_ending {
        if let Some(a) = audio_ending.checked_sub(current_duration) {
            std::thread::sleep(a)
        }
    }
}
fn output(text: &str, tooltip: &str) -> String {
    format!("{{\"text\": \"{}\", \"tooltip\": \"{}\"}}", text, tooltip)
}
fn find_player() -> Result<mpris::Player, ()> {
    mpris::PlayerFinder::new()
        .map_err(|x| {
            log::error!("Error when finding mpris player: {}", x);
        })?
        .find_active()
        .map_err(|x| {
            log::error!("Error when finding active player: {}", x);
        })
}

fn clean_up() {
    if std::path::Path::new("/tmp/lyrics").is_file() {
        std::fs::remove_file("/tmp/lyrics").expect("Failed to remove file");
    }
}
