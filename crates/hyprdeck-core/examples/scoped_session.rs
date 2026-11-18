//! Private stdio driver for the synthetic HTTPS proof. All callers are the
//! fixture controller, which owns the browser and simulates the control plane.

use hyprdeck_core::collaboration::{Binding, Operation, Request, ScopedSession};
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    Grant { binding: Binding },
    Check { request: Request },
    Advance,
    Pause,
    Resume,
    Revoke,
}

fn grant(binding: Binding) -> ScopedSession {
    ScopedSession::new(
        binding,
        vec![
            Operation::ObserveVisual,
            Operation::ObserveSemantic,
            Operation::Activate,
        ],
        Instant::now() + Duration::from_secs(60),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = match std::env::args().nth(1).as_deref() {
        None => Some(grant(Binding {
            runtime: 1,
            task: 1,
            resource: 1,
        })),
        Some("--explicit-grant") => None,
        Some(_) => return Err("unexpected fixture argument".into()),
    };
    let mut out = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let denial = match (serde_json::from_str(&line?)?, &mut session) {
            (Input::Grant { binding }, None) => {
                session = Some(grant(binding));
                json!(null)
            }
            (Input::Grant { .. }, Some(_)) => json!("grant_exists"),
            (_, None) => json!("no_grant"),
            (Input::Check { request }, Some(session)) => {
                json!(session.check(request, Instant::now()).err())
            }
            (input, Some(session)) => {
                match input {
                    Input::Advance => session.advance_revision(),
                    Input::Pause => session.pause(),
                    Input::Resume => session.resume(),
                    Input::Revoke => session.revoke(),
                    _ => unreachable!(),
                }
                json!(null)
            }
        };
        writeln!(
            out,
            "{}",
            json!({ "revision": session.as_ref().map(ScopedSession::revision), "denial": denial })
        )?;
        out.flush()?;
    }
    Ok(())
}
