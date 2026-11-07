//! A proposer whose programs come from outside the process: a command that
//! reads a prompt on stdin and writes MAGE program bodies on stdout (plan 7.3).
//!
//! The protocol is small, so that a local model, a script, or a shim in front
//! of a remote API can all speak it:
//!
//! * the **prompt** arrives on the command's stdin: the grammar as the arena
//!   states it, the pool's best programs as `source()` with their rewards, and
//!   the request;
//! * the **reply** is on stdout: up to `n` program bodies, one per block, with
//!   blocks separated by lines containing only `---`. A block may also be a
//!   whole `source()` text. [`Program::parse`] takes either.
//!
//! ## No bypass
//!
//! A command agent is a [`Proposer`] like any other, and its only output is
//! [`Candidate`]s. Those go through the same gate (`Substrate::prepare`), the
//! same fuel, the same canary check, the same novelty and credit, and the same
//! energy span and budget as every other agent's programs. This module touches
//! none of them: no pool, no learner, no ledger.
//!
//! ## Energy
//!
//! [`crate::arena::run_with`] wraps each agent's `propose` in its own energy
//! span, and charges that span to the agent, to its role and to its kind (the
//! ledger's `by_role` and `by_kind`, plan F2). The
//! command runs inside `propose`, so a local model behind it is metered by that
//! span, which is the local-model half of plan 3.4. Nothing here measures
//! energy itself.
//!
//! ## Budget cap
//!
//! `propose` never returns more than `n`, the kernel's share for the round. A
//! parsed block beyond that is counted as `truncated`; a block that does not
//! parse is counted as `unparsed`. A failed call proposes nothing and is counted
//! as `failed_calls`. The arena does not retry, so a broken command costs its
//! own span and nothing else.

use crate::agents::{Candidate, Pool, PoolEntry, Proposer, Scored};
use crate::grammar::{describe_grammar, Program, Rng};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Where a prompt goes and a reply comes from. The arena's only seam between
/// the proposer and whatever writes its programs.
pub trait Backend {
    fn complete(&mut self, prompt: &str) -> Result<String, String>;
}

/// How many of the pool's highest-reward entries the prompt shows.
pub const PROMPT_EXAMPLES: usize = 8;

/// A backend that runs a command: `argv[0]`, with the rest as its arguments.
pub struct CommandBackend {
    pub argv: Vec<String>,
    pub timeout: Duration,
}

impl Backend for CommandBackend {
    fn complete(&mut self, prompt: &str) -> Result<String, String> {
        let (program, args) = self.argv.split_first().ok_or("the proposer command is empty")?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start `{program}`: {e}"))?;
        // Every pipe is served on its own thread, so neither a full stdout nor
        // a command that never reads its prompt can block the timeout below.
        let stdin = child.stdin.take();
        let prompt = prompt.as_bytes().to_vec();
        let writer = std::thread::spawn(move || {
            if let Some(mut stdin) = stdin {
                // A command that exits without reading gets a broken pipe here.
                // Its exit status is what reports that, so the error is dropped.
                let _ = stdin.write_all(&prompt);
            }
        });
        let stdout = child.stdout.take().map(read_all);
        let stderr = child.stderr.take().map(read_all);
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    // The pipe threads are left to finish on their own: the kill closed them.
                    return Err(format!("`{program}` ran past its {:?} timeout and was killed", self.timeout));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("waiting for `{program}`: {e}"));
                }
            }
        };
        let _ = writer.join();
        let stdout = stdout.and_then(|h| h.join().ok()).unwrap_or_default();
        let stderr = stderr.and_then(|h| h.join().ok()).unwrap_or_default();
        if !status.success() {
            let stderr = String::from_utf8_lossy(&stderr);
            return Err(format!("`{program}` exited with {status}: {}", tail(stderr.trim(), 400)));
        }
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    }
}

fn read_all<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    })
}

/// The last `max` characters of `s`.
fn tail(s: &str, max: usize) -> &str {
    let n = s.chars().count();
    if n <= max {
        return s;
    }
    let start = s.char_indices().nth(n - max).map_or(0, |(i, _)| i);
    &s[start..]
}

/// The blocks of a reply: the text between lines that are exactly `---`, with
/// blank blocks dropped.
pub fn split_blocks(reply: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut cur = String::new();
    for line in reply.lines() {
        if line.trim() == "---" {
            blocks.push(std::mem::take(&mut cur));
        } else {
            cur.push_str(line);
            cur.push('\n');
        }
    }
    blocks.push(cur);
    blocks.into_iter().filter(|b| !b.trim().is_empty()).collect()
}

/// The prompt for a round that asks for `n` programs, showing `top`, the pool's
/// best entries, as `source()` with their rewards.
///
/// The prompt ends on a clean token boundary: exactly one newline after the
/// final instruction line, with no trailing space and no partial line. A
/// tokenized model's next-token distribution is skewed by a prompt that ends
/// mid-token, so the last line is always the whole instruction. See Cartesia,
/// *From Tokens to Bytes, Part II*: https://www.cartesia.ai/blog/from-tokens-to-bytes-part-2
/// (plan F3).
pub fn build_prompt(n: usize, top: &[&PoolEntry]) -> String {
    let sources: Vec<(f64, String)> = top.iter().map(|e| (e.reward, e.program.source())).collect();
    let shown: Vec<(f64, &str)> = sources.iter().map(|(r, s)| (*r, s.as_str())).collect();
    render_prompt(n, &shown)
}

/// The prompt text for [`build_prompt`], from entries given as reward and
/// source text. Each source is written without its trailing whitespace, so no
/// line of the prompt ends in a space, whatever the source held.
fn render_prompt(n: usize, top: &[(f64, &str)]) -> String {
    let mut p = String::from(
        "You write programs for the arena, a search over small typed MAGE programs. \
         A body is one list expression over the seed `s`. Write bodies only: no `f gen`, \
         no braces, no comments.\n\n",
    );
    p.push_str(&describe_grammar());
    p.push_str("\nThe best programs found so far, as written, with the learning progress each earned (higher is better):\n\n");
    if top.is_empty() {
        p.push_str("(none yet)\n\n");
    }
    for (reward, source) in top {
        p.push_str(&format!("reward {:.4}:\n{}\n", reward, source.trim_end()));
    }
    p.push_str(&format!(
        "Write up to {n} new programs, different from these. Each is a single body expression. \
         Put one per block, and separate blocks with a line containing only ---. Write nothing else.\n"
    ));
    p
}

/// What the agent has done, as `describe` reports it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Counts {
    /// Backend calls made.
    pub calls: usize,
    /// Backend calls that returned an error.
    pub failed_calls: usize,
    /// Non-blank blocks in the replies.
    pub blocks: usize,
    /// Blocks that parsed as programs, before the cap.
    pub parsed: usize,
    /// Blocks that did not parse.
    pub unparsed: usize,
    /// Parsed programs dropped to keep a round within its share.
    pub truncated: usize,
}

/// A proposer backed by a [`Backend`], typically a [`CommandBackend`].
pub struct CommandAgent {
    pub id: String,
    backend: Box<dyn Backend>,
    counts: Counts,
    last_error: Option<String>,
    last_parse_error: Option<String>,
    last_batch_reward: Option<f64>,
}

impl CommandAgent {
    pub fn new(id: impl Into<String>, backend: Box<dyn Backend>) -> Self {
        CommandAgent {
            id: id.into(),
            backend,
            counts: Counts::default(),
            last_error: None,
            last_parse_error: None,
            last_batch_reward: None,
        }
    }

    pub fn counts(&self) -> &Counts {
        &self.counts
    }
}

impl Proposer for CommandAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        "command"
    }

    fn propose(&mut self, n: usize, pool: &Pool, _rng: &mut Rng) -> Vec<Candidate> {
        // A round with no share asks for nothing, and so costs no call.
        if n == 0 {
            return Vec::new();
        }
        let top = pool.top(PROMPT_EXAMPLES);
        let prompt = build_prompt(n, &top);
        self.counts.calls += 1;
        let reply = match self.backend.complete(&prompt) {
            Ok(reply) => reply,
            Err(why) => {
                self.counts.failed_calls += 1;
                self.last_error = Some(why);
                return Vec::new();
            }
        };
        let mut out = Vec::new();
        for block in split_blocks(&reply) {
            self.counts.blocks += 1;
            match Program::parse(&block) {
                Ok(program) => {
                    self.counts.parsed += 1;
                    if out.len() < n {
                        out.push(Candidate { program, trace: None, parents: vec![] });
                    } else {
                        self.counts.truncated += 1;
                    }
                }
                Err(why) => {
                    self.counts.unparsed += 1;
                    self.last_parse_error = Some(why);
                }
            }
        }
        out
    }

    fn feedback(&mut self, scored: &[Scored]) {
        // A refused program earns nothing, as for every agent.
        if scored.is_empty() {
            return;
        }
        let sum: f64 = scored.iter().map(|(_, r)| r.unwrap_or(0.0)).sum();
        self.last_batch_reward = Some(sum / scored.len() as f64);
    }

    fn describe(&self) -> serde_json::Value {
        let c = &self.counts;
        serde_json::json!({
            "calls": c.calls,
            "failed_calls": c.failed_calls,
            "blocks": c.blocks,
            "parsed": c.parsed,
            "unparsed": c.unparsed,
            "truncated": c.truncated,
            "last_batch_mean_reward": self.last_batch_reward,
            "last_error": self.last_error,
            "last_parse_error": self.last_parse_error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::ListExpr;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Replies with a fixed text, or fails, and records every prompt it was given.
    struct Fake {
        reply: Result<String, String>,
        prompts: Rc<RefCell<Vec<String>>>,
    }

    impl Backend for Fake {
        fn complete(&mut self, prompt: &str) -> Result<String, String> {
            self.prompts.borrow_mut().push(prompt.to_string());
            self.reply.clone()
        }
    }

    fn agent(reply: Result<&str, &str>) -> (CommandAgent, Rc<RefCell<Vec<String>>>) {
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let fake = Fake { reply: reply.map(str::to_string).map_err(str::to_string), prompts: prompts.clone() };
        (CommandAgent::new("cmd-0", Box::new(fake)), prompts)
    }

    fn propose(a: &mut CommandAgent, n: usize) -> Vec<Candidate> {
        a.propose(n, &Pool::new(16), &mut Rng(1))
    }

    #[test]
    fn blocks_are_split_on_separator_lines_and_parsed() {
        let reply = "range(3)\n---\nflatten([range(3); 2])\n---\n@role(candidate)\nf gen(s: usize) -> [usize] {\n    range(4)\n}\n";
        let (mut a, _) = agent(Ok(reply));
        let out = propose(&mut a, 5);
        let bodies: Vec<ListExpr> = out.iter().map(|c| c.program.body.clone()).collect();
        assert_eq!(bodies.len(), 3);
        assert_eq!(bodies[0], ListExpr::Range(3));
        assert!(matches!(bodies[1], ListExpr::Copy(..)));
        assert_eq!(bodies[2], ListExpr::Range(4));
        assert!(out.iter().all(|c| c.trace.is_none() && c.parents.is_empty()));
        let c = a.counts();
        assert_eq!((c.calls, c.blocks, c.parsed, c.unparsed, c.truncated), (1, 3, 3, 0, 0));
    }

    #[test]
    fn unparseable_blocks_are_counted_not_returned() {
        let reply = "range(3)\n---\nhello there\n---\nrange(2).map(|x| y)\n---\n";
        let (mut a, _) = agent(Ok(reply));
        let out = propose(&mut a, 5);
        assert_eq!(out.len(), 1);
        let c = a.counts();
        assert_eq!((c.blocks, c.parsed, c.unparsed), (3, 1, 2));
        assert!(a.describe()["last_parse_error"].as_str().is_some_and(|e| e.contains("not bound")));
    }

    /// The budget cap: a reply with more programs than the share gets the share.
    #[test]
    fn more_than_n_programs_are_truncated_to_n() {
        let reply = (1..=5).map(|k| format!("range({k})")).collect::<Vec<_>>().join("\n---\n");
        let (mut a, _) = agent(Ok(&reply));
        let out = propose(&mut a, 2);
        assert_eq!(out.len(), 2, "the cap is the share");
        assert_eq!(out[0].program.body, ListExpr::Range(1));
        let c = a.counts();
        assert_eq!((c.parsed, c.truncated), (5, 3));
    }

    #[test]
    fn a_backend_error_proposes_nothing_and_counts_a_failed_call() {
        let (mut a, _) = agent(Err("the model is down"));
        assert!(propose(&mut a, 4).is_empty());
        let c = a.counts();
        assert_eq!((c.calls, c.failed_calls), (1, 1));
        assert!(a.describe()["last_error"].as_str().is_some_and(|e| e.contains("model is down")));
    }

    #[test]
    fn a_zero_share_asks_for_nothing() {
        let (mut a, prompts) = agent(Ok("range(3)"));
        assert!(propose(&mut a, 0).is_empty());
        assert_eq!(a.counts().calls, 0);
        assert!(prompts.borrow().is_empty());
    }

    #[test]
    fn the_prompt_carries_the_pools_best_sources_and_the_grammar() {
        let mut pool = Pool::new(32);
        // Ten programs, rewards 0.1..1.0 by their length: the top eight are
        // `range(3)`..`range(10)`, and `range(1)` and `range(2)` are not shown.
        for k in 1..=10u64 {
            pool.add(Program { body: ListExpr::Range(k) }, vec![0], k as f64 / 10.0, "other", 0);
        }
        let (mut a, prompts) = agent(Ok("range(5)"));
        a.propose(3, &pool, &mut Rng(1));
        let prompt = prompts.borrow()[0].clone();
        for k in 3..=10u64 {
            assert!(prompt.contains(&Program { body: ListExpr::Range(k) }.source()), "range({k}) missing");
        }
        for k in 1..=2u64 {
            assert!(!prompt.contains(&Program { body: ListExpr::Range(k) }.source()), "range({k}) shown");
        }
        assert!(prompt.contains("reward 1.0000:"));
        assert!(prompt.contains("wrapping_add"), "the grammar is described");
        assert!(prompt.contains("Write up to 3 new programs"));
    }

    #[test]
    fn feedback_records_the_mean_reward_of_the_last_batch() {
        let (mut a, _) = agent(Ok(""));
        let p = Program { body: ListExpr::Range(2) };
        let batch = vec![
            (Candidate { program: p.clone(), trace: None, parents: vec![] }, Some(2.0)),
            (Candidate { program: p, trace: None, parents: vec![] }, None),
        ];
        a.feedback(&batch);
        assert_eq!(a.describe()["last_batch_mean_reward"], serde_json::json!(1.0));
    }

    #[test]
    fn split_blocks_drops_blank_blocks_and_keeps_bodies_whole() {
        let blocks = split_blocks("\n---\nrange(2)\n  \n---\n---\n[1;\n2]\n");
        assert_eq!(blocks, vec!["range(2)\n  \n".to_string(), "[1;\n2]\n".to_string()]);
    }

    /// The prompt ends on exactly one newline after a last line with no
    /// trailing whitespace (plan F3).
    fn assert_clean_boundary(prompt: &str) {
        assert!(prompt.ends_with('\n'), "the prompt does not end on a newline: {prompt:?}");
        assert!(!prompt.ends_with("\n\n"), "the prompt ends on a blank line: {prompt:?}");
        let last = prompt.lines().last().expect("a prompt has a last line");
        assert_eq!(last.trim_end(), last, "the last line has trailing whitespace: {last:?}");
        assert!(last.starts_with("Write up to "), "the last line is not the instruction: {last:?}");
    }

    #[test]
    fn the_prompt_ends_on_a_clean_boundary_for_every_pool_size() {
        // Empty, one entry, a full prompt's worth, and more than it shows.
        for k in [0usize, 1, PROMPT_EXAMPLES, PROMPT_EXAMPLES + 4] {
            let mut pool = Pool::new(32);
            for i in 0..k {
                pool.add(Program { body: ListExpr::Range(i as u64 + 1) }, vec![0], i as f64 + 1.0, "other", 0);
            }
            let (mut a, prompts) = agent(Ok("range(5)"));
            a.propose(3, &pool, &mut Rng(1));
            let prompt = prompts.borrow()[0].clone();
            assert_clean_boundary(&prompt);
            if k == 0 {
                assert!(prompt.contains("(none yet)\n\n"), "an empty pool says so");
            }
        }
    }

    #[test]
    fn an_entry_with_trailing_whitespace_does_not_move_the_boundary() {
        // Sources from `source()` never end in whitespace, so this is for a
        // source that does: it is shown without it, and the prompt still ends
        // on the instruction.
        for shown in [
            vec![(0.5, "range(2)  \n\n")],
            vec![(0.5, "range(2)\t"), (0.2, "range(3) \n")],
            vec![],
        ] {
            let prompt = render_prompt(2, &shown);
            assert_clean_boundary(&prompt);
            for (_, source) in &shown {
                assert!(prompt.contains(&format!("{}\n", source.trim_end())), "{source:?} is not shown trimmed");
            }
        }
    }
}
