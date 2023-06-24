
#[derive(Debug)]
pub enum LemmySearchError {
    Unknown,
    Database(postgres::Error),
    DatabaseConnection(r2d2_postgres::r2d2::Error),
    Network(reqwest::Error)
}

impl std::fmt::Display for LemmySearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Unknown => write!(f, "Unknown Error"),
            Self::Database(postgres) => postgres.fmt(f),
            Self::DatabaseConnection(r2d2_postgres) => r2d2_postgres.fmt(f),
            Self::Network(reqwest) => reqwest.fmt(f)
        }
    }
}

impl From<postgres::Error> for LemmySearchError {
    fn from(value: postgres::Error) -> Self {
        LemmySearchError::Database(value)
    }
}


impl From<r2d2_postgres::r2d2::Error> for LemmySearchError {
    fn from(value: r2d2_postgres::r2d2::Error) -> Self {
        LemmySearchError::DatabaseConnection(value)
    }
}

impl From<reqwest::Error> for LemmySearchError {
    fn from(value: reqwest::Error) -> Self {
        LemmySearchError::Network(value)
    }
}

pub trait LogError<T> {
    fn log_error(
        self, 
        message : 
        &str, log : bool
    ) -> Result<T, LemmySearchError>;
}

impl<T> LogError<T> for Result<T, LemmySearchError> {
    fn log_error(
        self, 
        message : &str, 
        log : bool
    ) -> Result<T, LemmySearchError> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => {
                println!("{}", message);
                if log {
                    println!("{}", err);
                }
                Err(err)
            }
        }
    }
}
