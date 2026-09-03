use crate::{Error, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

/// Explicit config sources.
///
/// The process environment is never snapshotted. When it is enabled, only the
/// environment variables declared by the schema are read, one at a time.
#[derive(Debug, Clone)]
pub struct Sources {
    args: Vec<OsString>,
    env: BTreeMap<String, String>,
    process_env: bool,
    config_path: Option<PathBuf>,
    config_value: Option<Value>,
}

impl Sources {
    /// Current process argv and environment.
    pub fn new() -> Self {
        Self {
            args: env::args_os().collect(),
            env: BTreeMap::new(),
            process_env: true,
            config_path: None,
            config_value: None,
        }
    }

    /// No argv, no environment, and no config file.
    pub fn empty() -> Self {
        Self {
            args: Vec::new(),
            env: BTreeMap::new(),
            process_env: false,
            config_path: None,
            config_value: None,
        }
    }

    /// Explicit argv (including the program name) plus the process environment.
    ///
    /// Use [`Sources::empty`] with [`Sources::with_args`] for argv-only parsing.
    #[allow(clippy::should_implement_trait)]
    pub fn from_iter<I, T>(args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        Self::new().with_args(args)
    }

    /// Replace argv. The first element is the program name.
    pub fn with_args<I, T>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Add or replace one env binding. Explicit bindings win over the process environment.
    pub fn with_env_pair(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Add or replace many env bindings.
    pub fn with_env<I, K, V>(mut self, pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (key, value) in pairs {
            self.env.insert(key.into(), value.into());
        }
        self
    }

    /// Enable or disable reading declared env bindings from the process environment.
    pub fn with_process_env(mut self, enabled: bool) -> Self {
        self.process_env = enabled;
        self
    }

    /// Set a config file path.
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Set an already-loaded config value.
    pub fn with_config_value(mut self, value: Value) -> Self {
        self.config_value = Some(value);
        self
    }

    /// argv, including the program name.
    pub fn args(&self) -> &[OsString] {
        &self.args
    }

    /// Explicit env bindings.
    pub fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    /// Whether declared env bindings are read from the process environment.
    pub fn process_env(&self) -> bool {
        self.process_env
    }

    /// The config file path, if any.
    pub fn config_path(&self) -> Option<&PathBuf> {
        self.config_path.as_ref()
    }

    /// The pre-loaded config value, if any.
    pub fn config_value(&self) -> Option<&Value> {
        self.config_value.as_ref()
    }

    pub(crate) fn take_config_path(&mut self) -> Option<PathBuf> {
        self.config_path.take()
    }

    pub(crate) fn take_config_value(&mut self) -> Option<Value> {
        self.config_value.take()
    }

    /// Look up one declared env binding.
    pub(crate) fn env_value(&self, name: &str) -> Result<Option<String>> {
        if let Some(value) = self.env.get(name) {
            return Ok(Some(value.clone()));
        }
        if !self.process_env {
            return Ok(None);
        }
        match env::var_os(name) {
            None => Ok(None),
            Some(value) => value.into_string().map(Some).map_err(|_| {
                Error::Message(format!("environment variable {name} is not valid UTF-8"))
            }),
        }
    }
}

impl Default for Sources {
    fn default() -> Self {
        Self::new()
    }
}
