//! `selfmod`: drive the self-modification workload from the command line.
//! All of the logic is in [`germline::selfmod::cli`]; this is the wrapper.
//!
//! The last line of stdout is always a JSON report. Exit status: 0 when the run
//! was carried out (a refused candidate is a normal result), 1 when it could not
//! start or resume (the report is `{"ok":false,"error":...}`), 3 when it started
//! and halted on a workload failure.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match germline::selfmod::cli::run(&args) {
        Ok(out) => {
            println!("{out}");
            let failed = out
                .lines()
                .last()
                .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .is_some_and(|v| v["ok"] == false);
            if failed {
                std::process::exit(3);
            }
        }
        Err(e) => {
            eprintln!("selfmod: {e}");
            println!("{}", serde_json::json!({ "ok": false, "error": e }));
            std::process::exit(1);
        }
    }
}
