//! WebSocket handshake control.

pub mod headers;
pub mod client;
pub mod server;

mod machine;

use std::io::{Read, Write};

use base64;
use sha1::Sha1;

use error::Error;
use protocol::WebSocket;

use self::machine::{HandshakeMachine, RoundResult, StageResult, TryParse};

/// A WebSocket handshake.
pub struct MidHandshake<Stream, Role> {
    role: Role,
    machine: HandshakeMachine<Stream>,
}

impl<Stream, Role> MidHandshake<Stream, Role> {
    /// Returns a shared reference to the inner stream.
    pub fn get_ref(&self) -> &Stream {
        self.machine.get_ref()
    }
    /// Returns a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut Stream {
        self.machine.get_mut()
    }
}

impl<Stream: Read + Write, Role: HandshakeRole> MidHandshake<Stream, Role> {
    /// Restarts the handshake process.
    pub fn handshake(self) -> Result<WebSocket<Stream>, HandshakeError<Stream, Role>> {
        let mut mach = self.machine;
        loop {
            mach = match mach.single_round()? {
                RoundResult::WouldBlock(m) => {
                    return Err(HandshakeError::Interrupted(MidHandshake { machine: m, ..self }))
                }
                RoundResult::Incomplete(m) => m,
                RoundResult::StageFinished(s) => {
                    match self.role.stage_finished(s)? {
                        ProcessingResult::Continue(m) => m,
                        ProcessingResult::Done(ws) => return Ok(ws),
                    }
                }
            }
        }
    }
}

/// A handshake result.
pub enum HandshakeError<Stream, Role> {
    /// Handshake was interrupted (would block).
    Interrupted(MidHandshake<Stream, Role>),
    /// Handshake failed.
    Failure(Error),
}

impl<Stream, Role> From<Error> for HandshakeError<Stream, Role> {
    fn from(err: Error) -> Self {
        HandshakeError::Failure(err)
    }
}

/// Handshake role.
pub trait HandshakeRole {
    #[doc(hidden)]
    type IncomingData: TryParse;
    #[doc(hidden)]
    fn stage_finished<Stream>(&self, finish: StageResult<Self::IncomingData, Stream>)
        -> Result<ProcessingResult<Stream>, Error>;
}

/// Stage processing result.
#[doc(hidden)]
pub enum ProcessingResult<Stream> {
    Continue(HandshakeMachine<Stream>),
    Done(WebSocket<Stream>),
}

/// Turns a Sec-WebSocket-Key into a Sec-WebSocket-Accept.
fn convert_key(input: &[u8]) -> Result<String, Error> {
    // ... field is constructed by concatenating /key/ ...
    // ... with the string "258EAFA5-E914-47DA-95CA-C5AB0DC85B11" (RFC 6455)
    const WS_GUID: &'static [u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut sha1 = Sha1::new();
    sha1.update(input);
    sha1.update(WS_GUID);
    Ok(base64::encode(&sha1.digest().bytes()))
}

#[cfg(test)]
mod tests {

    use super::convert_key;

    #[test]
    fn key_conversion() {
        // example from RFC 6455
        assert_eq!(convert_key(b"dGhlIHNhbXBsZSBub25jZQ==").unwrap(),
                               "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

}
