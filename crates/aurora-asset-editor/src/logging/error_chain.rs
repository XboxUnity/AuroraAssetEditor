use std::error::Error;

/// The error's own message followed by the message of each `source()`, outermost first.
pub fn error_chain(error: &(dyn Error + 'static)) -> Vec<String> {
    let mut messages = vec![error.to_string()];
    let mut next = error.source();
    while let Some(source) = next {
        messages.push(source.to_string());
        next = source.source();
    }
    messages
}
