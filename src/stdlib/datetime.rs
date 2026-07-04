//! Date/time functions for Zenlang: `now()`, `format(date, fmt)`.
//!
//! `now()` returns the current Unix timestamp in seconds (as a float with
//! millisecond precision). `format(timestamp_secs, fmt)` formats a timestamp
//! into a human-readable string following `strftime`-style format specifiers:
//!
//! | Spec | Output |
//! |------|--------|
//! | `%Y`  | 4-digit year |
//! | `%m`  | 2-digit month |
//! | `%d`  | 2-digit day |
//! | `%H`  | 2-digit hour (00-23) |
//! | `%M`  | 2-digit minute |
//! | `%S`  | 2-digit second |
//! | `%%`  | literal `%` |
//!
//! # Examples
//! ```zen
//! let t = now();
//! print(format(t, "%Y-%m-%d")); // "2026-07-04"
//! print(format(t, "%H:%M:%S")); // "14:30:00"
//! ```

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Result;
use crate::error::Error;
use crate::value::Value;
use crate::vm::{VM, VMContext};

fn now_impl(_ctx: &mut VMContext, _args: &[Value]) -> Result<Value> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as f64;
    Ok(Value::Float(nanos / 1_000_000_000.0))
}

fn format_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let timestamp = match args.first() {
        Some(Value::Float(f)) => *f,
        Some(Value::Int(n)) => *n as f64,
        _ => {
            return Err(Error::Script {
                msg: "format() expects a numeric timestamp (seconds since epoch)".into(),
            })
        }
    };

    let fmt = match args.get(1) {
        Some(Value::Str(s)) => s.as_ref(),
        _ => {
            return Err(Error::Script {
                msg: "format() expects a format string as second argument".into(),
            })
        }
    };

    let secs = timestamp as u64;
    // UTC breakdown
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = (time_secs / 3600) as u32;
    let minutes = ((time_secs % 3600) / 60) as u32;
    let seconds = (time_secs % 60) as u32;

    // Days since Unix epoch to date
    let (year, month, day) = days_to_date(days);

    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('Y') => out.push_str(&format!("{:04}", year)),
                Some('m') => out.push_str(&format!("{:02}", month)),
                Some('d') => out.push_str(&format!("{:02}", day)),
                Some('H') => out.push_str(&format!("{:02}", hours)),
                Some('M') => out.push_str(&format!("{:02}", minutes)),
                Some('S') => out.push_str(&format!("{:02}", seconds)),
                Some('%') => out.push('%'),
                Some(other) => {
                    out.push('%');
                    out.push(other);
                }
                None => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }

    Ok(Value::Str(out.into()))
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_date(days: u64) -> (u64, u32, u32) {
    // Algorithm from Howard Hinnant
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

pub fn register(vm: &mut VM) {
    vm.register_native("now", Rc::new(now_impl));
    vm.register_native("format", Rc::new(format_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "now".into(),
            params: vec![],
            return_type: Some(Type::F64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "format".into(),
            params: vec![("timestamp".into(), Type::F64), ("fmt".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
    ]
}
