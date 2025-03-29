use mpris::DBusError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FormatError {
    #[error("Player has stopped. Tryning to find a new player..")]
    PlayerStopped,
    #[error("Error when fetching metadata: {0}")]
    MetadataError(DBusError),
    #[error("Failed to parse audio path.")]
    AudioParseError,
    #[error("Failed to find audio path: {0}")]
    AudioNotFoundError(Box<dyn std::error::Error>),
}