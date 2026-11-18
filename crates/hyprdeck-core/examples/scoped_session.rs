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
    Check { request: Request },
    Advance,
    Pause,
    Resume,
    Revoke,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = ScopedSession::new(
        Binding {
            runtime: 1,
            task: 1,
            resource: 1,
        },
        vec![
            Operation::ObserveVisual,
            Operation::ObserveSemantic,
            Operation::Activate,
        ],
        Instant::now() + Duration::from_secs(60),
    );
    let mut out = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let mut denial = None;
        match serde_json::from_str(&line?)? {
            Input::Check { request } => denial = session.check(request, Instant::now()).err(),
            Input::Advance => session.advance_revision(),
            Input::Pause => session.pause(),
            Input::Resume => session.resume(),
            Input::Revoke => session.revoke(),
        }
        writeln!(
            out,
            "{}",
            json!({ "revision": session.revision(), "denial": denial })
        )?;
        out.flush()?;
    }
    Ok(())
}
