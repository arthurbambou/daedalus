//! # Daedalus
//!
//! Daedalus is a library which provides models and methods to fetch metadata about games

#![warn(missing_docs, unused_import_braces, missing_debug_implementations)]

/// Models and methods for fetching metadata for the Fabric mod loader
pub mod fabric;
/// Models and methods for fetching metadata for Minecraft
pub mod minecraft;

#[derive(thiserror::Error, Debug)]
/// An error type representing possible errors when fetching metadata
pub enum Error {
    #[error("Failed to validate file checksum at url {url} with hash {hash} after {tries} tries")]
    /// A checksum was failed to validate for a file
    ChecksumFailure {
        /// The checksum's hash
        hash: String,
        /// The URL of the file attempted to be downloaded
        url: String,
        /// The amount of tries that the file was downloaded until failure
        tries: u32,
    },
    /// There was an error while deserializing metadata
    #[error("Error while deserializing JSON")]
    SerdeError(#[from] serde_json::Error),
    /// There was a network error when fetching an object
    #[error("Unable to fetch {item}")]
    FetchError {
        /// The internal reqwest error
        inner: reqwest::Error,
        /// The item that was failed to be fetched
        item: String,
    },
    /// There was an error when managing async tasks
    #[error("Error while managing asynchronous tasks")]
    TaskError(#[from] tokio::task::JoinError),
    /// Error while parsing input
    #[error("{0}")]
    ParseError(String),
}

/// Converts a maven artifact to a path
pub fn get_path_from_artifact(artifact: &str) -> Result<String, Error> {
    let name_items = artifact.split(':').collect::<Vec<&str>>();

    let package = name_items.get(0).ok_or_else(|| {
        Error::ParseError(format!("Unable to find package for library {}", &artifact))
    })?;
    let name = name_items.get(1).ok_or_else(|| {
        Error::ParseError(format!("Unable to find name for library {}", &artifact))
    })?;
    let version = name_items.get(2).ok_or_else(|| {
        Error::ParseError(format!("Unable to find version for library {}", &artifact))
    })?;

    Ok(format!(
        "{}/{}/{}-{}.jar",
        package.replace(".", "/"),
        version,
        name,
        version
    ))
}

/// Downloads a file with retry and checksum functionality
pub async fn download_file(url: &str, sha1: Option<&str>) -> Result<bytes::Bytes, Error> {
    let client = reqwest::Client::builder()
        .tcp_keepalive(Some(std::time::Duration::from_secs(10)))
        .build()
        .map_err(|err| Error::FetchError {
            inner: err,
            item: url.to_string(),
        })?;

    for attempt in 1..=4 {
        let result = client.get(url).send().await;

        match result {
            Ok(x) => {
                let bytes = x.bytes().await;

                if let Ok(bytes) = bytes {
                    if let Some(sha1) = sha1 {
                        if &*get_hash(bytes.clone()).await? != sha1 {
                            if attempt <= 3 {
                                continue;
                            } else {
                                return Err(Error::ChecksumFailure {
                                    hash: sha1.to_string(),
                                    url: url.to_string(),
                                    tries: attempt,
                                });
                            }
                        }
                    }

                    return Ok(bytes);
                } else if attempt <= 3 {
                    continue;
                } else if let Err(err) = bytes {
                    return Err(Error::FetchError {
                        inner: err,
                        item: url.to_string(),
                    });
                }
            }
            Err(_) if attempt <= 3 => continue,
            Err(err) => {
                return Err(Error::FetchError {
                    inner: err,
                    item: url.to_string(),
                })
            }
        }
    }

    unreachable!()
}

/// Computes a checksum of the input bytes
pub async fn get_hash(bytes: bytes::Bytes) -> Result<String, Error> {
    let hash = tokio::task::spawn_blocking(|| sha1::Sha1::from(bytes).hexdigest()).await?;

    Ok(hash)
}