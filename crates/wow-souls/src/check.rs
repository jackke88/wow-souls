//! `WOWSOULS_CHECK[=seconds]`: run for a bounded time, record every distinct ECS fault (a system
//! whose parameters cannot be met, a failed command), print them and exit non-zero if any. The
//! same gate as benilla's `WOW_WORLDVIEW_CHECK`, extended to our own systems: it catches a system
//! that needs a resource nothing inserts, a coupling no compiler sees.

use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

use bevy::ecs::error::{BevyError, ErrorContext};
use bevy::prelude::*;

/// How long a bare `WOWSOULS_CHECK` runs: long enough on a warm cache for every system to run.
const DEFAULT_SECS: f32 = 15.0;

/// Every distinct fault seen; a `static` because Bevy's error handler is a bare `fn` pointer.
static FAULTS: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// The check's wall-clock length in seconds.
#[derive(Resource, Clone, Copy)]
pub struct CheckDeadline(pub f32);

/// Arms the check when `WOWSOULS_CHECK` is set; returns whether it is armed.
pub fn configure(app: &mut App) -> Option<CheckDeadline> {
    let secs = parse_secs(std::env::var("WOWSOULS_CHECK").ok().as_deref())?;
    let deadline = CheckDeadline(secs);
    app.set_error_handler(record_fault)
        .insert_resource(deadline)
        .add_systems(Update, end_check);
    Some(deadline)
}

/// Unset is off; bare or unparseable is [`DEFAULT_SECS`].
fn parse_secs(v: Option<&str>) -> Option<f32> {
    let v = v?;
    Some(
        v.trim()
            .parse::<f32>()
            .ok()
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(DEFAULT_SECS),
    )
}

fn record_fault(error: BevyError, ctx: ErrorContext) {
    let entry = format!("{} `{}`: {error}", ctx.kind(), ctx.name());
    warn!("wow-souls check: {entry}");
    FAULTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(entry);
}

fn end_check(
    time: Res<Time<bevy::time::Real>>,
    deadline: Res<CheckDeadline>,
    mut exit: MessageWriter<AppExit>,
) {
    if time.elapsed_secs() >= deadline.0 {
        exit.write(AppExit::Success);
    }
}

/// Prints the verdict and makes it the exit code; a no-op when the check is off.
pub fn report(check: Option<CheckDeadline>, exit: AppExit) -> AppExit {
    if check.is_none() {
        return exit;
    }
    let faults = FAULTS.lock().unwrap_or_else(PoisonError::into_inner);
    if faults.is_empty() {
        println!("WOWSOULS_CHECK ok");
        return exit;
    }
    for f in faults.iter() {
        println!("WOWSOULS_CHECK fault {f}");
    }
    println!("WOWSOULS_CHECK {} fault(s)", faults.len());
    AppExit::error()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_is_off() {
        assert_eq!(parse_secs(None), None);
    }

    #[test]
    fn bare_or_bad_value_takes_the_default() {
        assert_eq!(parse_secs(Some("")), Some(DEFAULT_SECS));
        assert_eq!(parse_secs(Some("soon")), Some(DEFAULT_SECS));
        assert_eq!(parse_secs(Some("-3")), Some(DEFAULT_SECS));
    }

    #[test]
    fn seconds_parse() {
        assert_eq!(parse_secs(Some("40")), Some(40.0));
    }
}
