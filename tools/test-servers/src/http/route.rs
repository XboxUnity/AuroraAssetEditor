/// One canned answer: what `RecordedHttp` sends for a request to `path`.
#[derive(Debug, Clone)]
pub struct Route {
    pub path: String,
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
    pub behaviour: RouteBehaviour,
}

#[derive(Debug, Clone)]
pub enum RouteBehaviour {
    Normal,
    /// Announces `declared_len` bytes of body, sends the real (shorter) body and closes.
    Truncated {
        declared_len: usize,
    },
    /// Answers 302 with this `Location`; `{server}` stands for the server's base URL.
    Redirect {
        location: String,
    },
    /// Reads the request and then never answers.
    NeverRespond,
}
