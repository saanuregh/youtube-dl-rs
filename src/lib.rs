//! A crate for running and parsing the JSON output of `youtube-dl`.

#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
    rust_2018_idioms
)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub mod model;

pub use crate::model::*;

/// Data returned by `YoutubeDl::run`. Output can either be a single video or a playlist of videos.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum YoutubeDlOutput {
    /// Playlist result
    Playlist(Box<Playlist>),
    /// Single video result
    SingleVideo(Box<SingleVideo>),
}

impl YoutubeDlOutput {
    #[cfg(test)]
    fn to_single_video(self) -> SingleVideo {
        match self {
            YoutubeDlOutput::SingleVideo(video) => *video,
            _ => panic!("this is a playlist, not a single video"),
        }
    }
    #[cfg(test)]
    fn to_playlist(self) -> Playlist {
        match self {
            YoutubeDlOutput::Playlist(playlist) => *playlist,
            _ => panic!("this is a playlist, not a single video"),
        }
    }
}

/// Errors that can occur during executing `youtube-dl` or during parsing the output.
#[derive(Debug)]
pub enum Error {
    /// I/O error
    Io(std::io::Error),

    /// Error parsing JSON
    Json(serde_json::Error),

    /// `youtube-dl` returned a non-zero exit code
    ExitCode {
        /// Exit code
        code: i32,
        /// Standard error of youtube-dl
        stderr: String,
    },

    /// Process-level timeout expired.
    ProcessTimeout,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {}", err),
            Self::Json(err) => write!(f, "json error: {}", err),
            Self::ExitCode { code, stderr } => {
                write!(f, "non-zero exit code: {}, stderr: {}", code, stderr)
            }
            Self::ProcessTimeout => write!(f, "process timed out"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Json(err) => Some(err),
            Self::ExitCode { .. } => None,
            Self::ProcessTimeout => None,
        }
    }
}

/// A builder to create a `youtube-dl` command to execute.
#[derive(Clone, Debug)]
pub struct YoutubeDl {
    youtube_dl_path: Option<PathBuf>,
    format: Option<String>,
    flat_playlist: bool,
    socket_timeout: Option<String>,
    all_formats: bool,
    auth: Option<(String, String)>,
    user_agent: Option<String>,
    referer: Option<String>,
    query: String,
    process_timeout: Option<Duration>,
}

fn query_transform(query: String) -> String {
    if !query.starts_with("http") {
        return format!(r#"ytsearch1:{}"#, query);
    }
    query
}

impl YoutubeDl {
    /// Create a new builder.
    pub fn new<S: Into<String>>(query: S) -> Self {
        Self {
            query: query_transform(query.into()),
            youtube_dl_path: None,
            format: None,
            flat_playlist: false,
            socket_timeout: None,
            all_formats: false,
            auth: None,
            user_agent: None,
            referer: None,
            process_timeout: None,
        }
    }

    /// Set the path to the `youtube-dl` executable.
    pub fn youtube_dl_path<P: AsRef<Path>>(&mut self, youtube_dl_path: P) -> &mut Self {
        self.youtube_dl_path = Some(youtube_dl_path.as_ref().to_owned());
        self
    }

    /// Set the `-F` command line option.
    pub fn format<S: Into<String>>(&mut self, format: S) -> &mut Self {
        self.format = Some(format.into());
        self
    }

    /// Set the `--flat-playlist` command line flag.
    pub fn flat_playlist(&mut self, flat_playlist: bool) -> &mut Self {
        self.flat_playlist = flat_playlist;
        self
    }

    /// Set the `--socket-timeout` command line flag.
    pub fn socket_timeout<S: Into<String>>(&mut self, socket_timeout: S) -> &mut Self {
        self.socket_timeout = Some(socket_timeout.into());
        self
    }

    /// Set the `--user-agent` command line flag.
    pub fn user_agent<S: Into<String>>(&mut self, user_agent: S) -> &mut Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Set the `--referer` command line flag.
    pub fn referer<S: Into<String>>(&mut self, referer: S) -> &mut Self {
        self.referer = Some(referer.into());
        self
    }

    /// Set the `--all-formats` command line flag.
    pub fn all_formats(&mut self, all_formats: bool) -> &mut Self {
        self.all_formats = all_formats;
        self
    }

    /// Set the `-u` and `-p` command line flags.
    pub fn auth<S: Into<String>>(&mut self, username: S, password: S) -> &mut Self {
        self.auth = Some((username.into(), password.into()));
        self
    }

    /// Set a process-level timeout for youtube-dl. (this controls the maximum overall duration
    /// the process may take, when it times out, `Error::ProcessTimeout` is returned)
    pub fn process_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.process_timeout = Some(timeout);
        self
    }

    fn path(&self) -> &Path {
        match &self.youtube_dl_path {
            Some(path) => path,
            None => Path::new("youtube-dl"),
        }
    }

    fn process_args(&self) -> Vec<&str> {
        let mut args = vec![];
        if let Some(format) = &self.format {
            args.push("-f");
            args.push(format);
        }

        if self.flat_playlist {
            args.push("--flat-playlist");
        }

        if let Some(timeout) = &self.socket_timeout {
            args.push("--socket-timeout");
            args.push(timeout);
        }

        if self.all_formats {
            args.push("--all-formats");
        }

        if let Some((user, password)) = &self.auth {
            args.push("-u");
            args.push(user);
            args.push("-p");
            args.push(password);
        }

        if let Some(user_agent) = &self.user_agent {
            args.push("--user-agent");
            args.push(user_agent);
        }

        if let Some(referer) = &self.referer {
            args.push("--referer");
            args.push(referer);
        }
        args.push("-J");
        args.push(&self.query);
        log::debug!("youtube-dl arguments: {:?}", args);

        args
    }

    /// Run youtube-dl with the arguments specified through the builder.
    pub async fn run(&self) -> Result<YoutubeDlOutput, Error> {
        use serde_json::{json, Value};
        use std::process::Stdio;

        let process_args = self.process_args();
        let path = self.path();
        let mut child = tokio::process::Command::new(path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(process_args)
            .spawn()?;

        // Continually read from stdout so that it does not fill up with large output and hang forever.
        // We don't need to do this for stderr since only stdout has potentially giant JSON.
        let mut stdout = Vec::new();
        let child_stdout = child.stdout.take();
        tokio::io::copy(&mut child_stdout.unwrap(), &mut stdout).await?;

        let exit_code = if let Some(timeout) = self.process_timeout {
            match tokio::time::timeout(timeout, child).await {
                Ok(result) => match result {
                    Ok(status) => status,
                    Err(err) => {
                        return Err(Error::Io(err));
                    }
                },
                Err(_) => {
                    return Err(Error::ProcessTimeout);
                }
            }
        } else {
            child.await?
        };
        if exit_code.success() {
            let value: Value = serde_json::from_reader(stdout.as_slice())?;

            let is_playlist = value["_type"] == json!("playlist");
            if is_playlist {
                let playlist: Playlist = serde_json::from_value(value)?;
                Ok(YoutubeDlOutput::Playlist(Box::new(playlist)))
            } else {
                let video: SingleVideo = serde_json::from_value(value)?;
                Ok(YoutubeDlOutput::SingleVideo(Box::new(video)))
            }
        } else {
            Err(Error::ExitCode {
                code: exit_code.code().unwrap_or(1),
                stderr: "yrf".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::YoutubeDl;
    use std::time::Duration;

    #[tokio::test]
    async fn test_youtube_url() {
        let output = YoutubeDl::new("https://www.youtube.com/watch?v=7XGyWcuYVrg")
            .socket_timeout("15")
            .run()
            .await
            .unwrap()
            .to_single_video();
        assert_eq!(output.id, "7XGyWcuYVrg");
    }

    #[tokio::test]
    async fn test_with_timeout() {
        let output = YoutubeDl::new("https://www.youtube.com/watch?v=7XGyWcuYVrg")
            .socket_timeout("15")
            .process_timeout(Duration::from_secs(15))
            .run()
            .await
            .unwrap()
            .to_single_video();
        assert_eq!(output.id, "7XGyWcuYVrg");
    }

    #[tokio::test]
    async fn test_unknown_url() {
        YoutubeDl::new("https://www.rust-lang.org")
            .socket_timeout("15")
            .process_timeout(Duration::from_secs(15))
            .run()
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn test_playlist_timeout() {
        YoutubeDl::new("https://www.youtube.com/list=PLuDoiEqVUgejiZy0AOEEOLY2YFFXncwEA")
            .socket_timeout("15")
            .process_timeout(Duration::from_secs(15))
            .run()
            .await
            .unwrap()
            .to_playlist();
    }

    #[tokio::test]
    async fn test_search() {
        let output = YoutubeDl::new("Never Gonna Give You Up")
            .socket_timeout("15")
            .process_timeout(Duration::from_secs(15))
            .run()
            .await
            .unwrap()
            .to_playlist();
        assert_eq!(output.entries.unwrap().first().unwrap().id, "dQw4w9WgXcQ");
    }
}
