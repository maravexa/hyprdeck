//! Policy for one supervised, local collaboration session. A future broker
//! must authenticate its inputs and own the adapter, clock, and session state.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Identifiers assigned by the trusted broker. A runtime ID names one connection
/// lifetime, and a resource ID names one instance rather than a title or URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Binding {
    pub runtime: u64,
    pub task: u64,
    pub resource: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    ObserveVisual,
    ObserveSemantic,
    Activate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    Local,
    External,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Request {
    pub binding: Binding,
    pub revision: u64,
    pub operation: Operation,
    pub destination: Destination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Denial {
    Revoked,
    Paused,
    Expired,
    WrongBinding,
    StaleRevision,
    Destination,
    Operation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Active,
    Paused,
    Revoked,
}

/// Owned by the dispatcher, with exclusive access to the selected resource.
/// The dispatcher must enforce one writer across sessions. These checks do not
/// control the effects of an action already handed to the adapter.
pub struct ScopedSession {
    binding: Binding,
    revision: u64,
    operations: Vec<Operation>,
    expires_at: Instant,
    state: State,
}

impl ScopedSession {
    pub fn new(binding: Binding, operations: Vec<Operation>, expires_at: Instant) -> Self {
        Self {
            binding,
            revision: 0,
            operations,
            expires_at,
            state: State::Active,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// A focus or semantic-tree change makes earlier observations stale.
    /// Overflow closes the session so a revision is never reused.
    pub fn advance_revision(&mut self) {
        match self.revision.checked_add(1) {
            Some(revision) => self.revision = revision,
            None => self.revoke(),
        }
    }

    /// Human takeover blocks new dispatch and invalidates queued observations.
    pub fn pause(&mut self) {
        if self.state == State::Active {
            self.state = State::Paused;
            self.advance_revision();
        }
    }

    /// Only the human control path may resume the same resource instance.
    pub fn resume(&mut self) {
        if self.state == State::Paused {
            self.state = State::Active;
        }
    }

    /// Use for navigation outside the grant, replacement, lock, or disconnect.
    pub fn revoke(&mut self) {
        self.state = State::Revoked;
    }

    /// Evaluate at admission and again at the dispatch boundary, using broker-
    /// sampled monotonic time. A positive result is valid only at that boundary.
    pub fn check(&self, request: Request, now: Instant) -> Result<(), Denial> {
        match self.state {
            State::Active => (),
            State::Paused => return Err(Denial::Paused),
            State::Revoked => return Err(Denial::Revoked),
        }
        if now >= self.expires_at {
            return Err(Denial::Expired);
        }
        if request.binding != self.binding {
            return Err(Denial::WrongBinding);
        }
        if request.revision != self.revision {
            return Err(Denial::StaleRevision);
        }
        if request.destination != Destination::Local {
            return Err(Denial::Destination);
        }
        if !self.operations.contains(&request.operation) {
            return Err(Denial::Operation);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn fixture() -> (ScopedSession, Request, Instant) {
        let binding = Binding {
            runtime: 1,
            task: 2,
            resource: 3,
        };
        let now = Instant::now();
        let session = ScopedSession::new(
            binding,
            vec![Operation::ObserveSemantic],
            now + Duration::from_secs(1),
        );
        let request = Request {
            binding,
            revision: 0,
            operation: Operation::ObserveSemantic,
            destination: Destination::Local,
        };
        (session, request, now)
    }

    #[test]
    fn checks_each_binding_operation_and_destination_and_expires_at_the_deadline() {
        let (session, request, now) = fixture();
        assert_eq!(session.check(request, now), Ok(()));
        let Binding {
            runtime,
            task,
            resource,
        } = request.binding;
        for binding in [
            Binding {
                runtime: 4,
                task,
                resource,
            },
            Binding {
                runtime,
                task: 4,
                resource,
            },
            Binding {
                runtime,
                task,
                resource: 4,
            },
        ] {
            assert_eq!(
                session.check(Request { binding, ..request }, now),
                Err(Denial::WrongBinding)
            );
        }
        assert_eq!(
            session.check(
                Request {
                    operation: Operation::Activate,
                    ..request
                },
                now
            ),
            Err(Denial::Operation)
        );
        assert_eq!(
            session.check(
                Request {
                    destination: Destination::External,
                    ..request
                },
                now
            ),
            Err(Denial::Destination)
        );
        assert_eq!(
            session.check(request, now + Duration::from_secs(1)),
            Err(Denial::Expired)
        );
    }

    #[test]
    fn rechecks_queued_actions_after_focus_takeover_and_revocation() {
        let (mut session, mut request, now) = fixture();
        assert_eq!(session.check(request, now), Ok(()));
        session.advance_revision();
        assert_eq!(session.check(request, now), Err(Denial::StaleRevision));
        request.revision = session.revision();
        assert_eq!(session.check(request, now), Ok(()));
        session.pause();
        assert_eq!(session.check(request, now), Err(Denial::Paused));
        session.resume();
        assert_eq!(session.check(request, now), Err(Denial::StaleRevision));
        request.revision = session.revision();
        assert_eq!(session.check(request, now), Ok(()));
        session.revoke();
        session.resume();
        assert_eq!(session.check(request, now), Err(Denial::Revoked));
    }
}
