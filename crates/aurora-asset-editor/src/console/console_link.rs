/// What the connection chip shows about the console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleLink {
    NotSetUp,
    /// Settings exist, but no connection has been tried with them yet.
    Unknown,
    Connected {
        revision: String,
    },
    Failed,
}
