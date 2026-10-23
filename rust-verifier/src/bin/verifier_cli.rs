//! Thin CLI around the verifier library.
//!
//! Usage:
//!     verifier-cli <spec.json> <submission.json>
//!     cat submission.json | verifier-cli <spec.json> -
//!
//! On success it prints the `Verdict` as JSON to stdout and exits 0 when the
//! submission passed, 2 when it failed verification, and 3 on a structural
//! error. The Node.js backend consumes this contract directly.

use std::io::Read;
use std::process::ExitCode;

use proof_gated_escrow::verify_json;

fn read_source(arg: &str) -> std::io::Result<String> {
    if arg == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        std::fs::read_to_string(arg)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: verifier-cli <spec.json> <submission.json|->");
        return ExitCode::from(64); // EX_USAGE
    }

    let spec_json = match read_source(&args[1]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read spec `{}`: {e}", args[1]);
            return ExitCode::from(66); // EX_NOINPUT
        }
    };
    let submission_json = match read_source(&args[2]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read submission `{}`: {e}", args[2]);
            return ExitCode::from(66);
        }
    };

    match verify_json(&spec_json, &submission_json) {
        Ok(verdict) => {
            println!("{}", serde_json::to_string_pretty(&verdict).unwrap());
            if verdict.passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(e) => {
            // Emit a structured error so the backend doesn't scrape stderr text.
            let payload = serde_json::json!({ "error": e.to_string() });
            eprintln!("{payload}");
            ExitCode::from(3)
        }
    }
}
