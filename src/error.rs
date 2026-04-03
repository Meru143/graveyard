use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl StdError for ConfigError {}

#[derive(Debug)]
pub struct UsageError {
    message: String,
}

impl UsageError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl StdError for UsageError {}

pub fn exit_code(error: &anyhow::Error) -> i32 {
    if error.downcast_ref::<ConfigError>().is_some() || error.downcast_ref::<UsageError>().is_some()
    {
        2
    } else {
        3
    }
}
