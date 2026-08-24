use std::error::Error;
use std::fmt::{Display, Formatter};

pub type NexusResult<T> = Result<T, NexusError>;

#[derive(Debug)]
pub enum NexusError {
    EmptyUrl,
    UnsupportedScheme(String),
    InvalidUrl(url::ParseError),
    InvalidInput(String),
    Network(reqwest::Error),
    Io(std::io::Error),
    BodyTooLarge { limit: usize, actual: usize },
    Security(String),
    Storage(String),
    Layout(String),
    Render(String),
}

impl Display for NexusError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyUrl => write!(f, "URL vuoto"),
            Self::UnsupportedScheme(scheme) => {
                write!(f, "schema URL non supportato in Nexus 0.20: {scheme}")
            }
            Self::InvalidUrl(err) => write!(f, "URL non valido: {err}"),
            Self::InvalidInput(message) => write!(f, "input non valido: {message}"),
            Self::Network(err) => write!(f, "errore di rete: {err}"),
            Self::Io(err) => write!(f, "errore I/O: {err}"),
            Self::Security(message) => write!(f, "errore sicurezza browser: {message}"),
            Self::Storage(message) => write!(f, "errore Web Storage: {message}"),
            Self::BodyTooLarge { limit, actual } => write!(
                f,
                "risposta troppo grande: {actual} byte (limite Nexus 0.20: {limit} byte)"
            ),
            Self::Layout(message) => write!(f, "errore layout: {message}"),
            Self::Render(message) => write!(f, "errore rendering: {message}"),
        }
    }
}

impl Error for NexusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidUrl(err) => Some(err),
            Self::Network(err) => Some(err),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<url::ParseError> for NexusError {
    fn from(value: url::ParseError) -> Self {
        Self::InvalidUrl(value)
    }
}

impl From<reqwest::Error> for NexusError {
    fn from(value: reqwest::Error) -> Self {
        Self::Network(value)
    }
}

impl From<std::io::Error> for NexusError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
