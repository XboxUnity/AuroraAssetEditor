use std::any::Any;
use std::error::Error;
use std::fmt;

/// A job that panicked; it reads as the panic's message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobPanic {
    message: String,
}

impl JobPanic {
    /// `payload` is what `catch_unwind` caught.
    pub fn from_payload(payload: &(dyn Any + Send)) -> JobPanic {
        let message = if let Some(text) = payload.downcast_ref::<&str>() {
            (*text).to_owned()
        } else if let Some(text) = payload.downcast_ref::<String>() {
            text.clone()
        } else {
            "the job panicked without a message".to_owned()
        };
        JobPanic { message }
    }
}

impl fmt::Display for JobPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for JobPanic {}
