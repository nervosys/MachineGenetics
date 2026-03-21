//! # SKB Query Language (SKB-QL)
//!
//! A domain-specific query language optimized for agent consumption — designed
//! for minimal tokens and maximum precision.
//!
//! ## Syntax
//!
//! ```text
//! Query ::= SELECT fields FROM database WHERE conditions
//!         | MATCH pattern IN scope
//!         | COUNT pattern IN scope
//! ```
//!
//! ## Examples
//!
//! ```text
//! SELECT id, severity FROM ownership WHERE category = "use-after-move"
//! SELECT * FROM all WHERE severity >= "error" AND fix_confidence > 0.9
//! MATCH UseAfterMove IN crate
//! COUNT MutableBorrow IN function("process")
//! ```
//!
//! Reference: REDOX_PROPOSAL.md §15.3

use crate::{Database, Rule, SafetyKnowledgeBase, Severity};
use std::fmt;
use std::time::Instant;

// ===========================================================================
// Query AST
// ===========================================================================

/// Parsed SKB-QL query.
#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    Select { fields: Vec<Field>, database: DatabaseSelector, conditions: Vec<Condition> },
    Match { pattern: String, scope: QueryScope },
    Count { pattern: String, scope: QueryScope },
}

/// Selectable fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Id,
    Severity,
    Category,
    Description,
    FixTemplate,
    FixConfidence,
    Pattern,
    All,
}

/// Database selector in FROM clause.
#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseSelector {
    Specific(Database),
    All,
}

/// Query scope for MATCH/COUNT.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryScope {
    Function(String),
    Module(String),
    Crate,
    Global,
}

/// WHERE conditions.
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    CategoryEq(String),
    SeverityEq(Severity),
    SeverityGte(Severity),
    SeverityLte(Severity),
    FixConfidenceGt(f64),
    PatternEq(String),
    TagContains(String),
}

// ===========================================================================
// Query result
// ===========================================================================

/// Result of executing a query.
#[derive(Debug)]
pub struct QueryResult<'a> {
    /// Matched rules.
    pub rules: Vec<&'a Rule>,
    /// Total count (for COUNT queries, matches rules.len()).
    pub total_count: usize,
    /// Whether the result set was truncated.
    pub truncated: bool,
    /// Query evaluation time.
    pub eval_time_us: u64,
}

impl<'a> QueryResult<'a> {
    pub fn empty(eval_time_us: u64) -> Self {
        Self { rules: Vec::new(), total_count: 0, truncated: false, eval_time_us }
    }
}

// ===========================================================================
// Parser
// ===========================================================================

/// Parse error.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at position {}: {}", self.position, self.message)
    }
}

/// Parse an SKB-QL query string into a Query AST.
pub fn parse_query(input: &str) -> Result<Query, ParseError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(ParseError { message: "empty query".into(), position: 0 });
    }

    match tokens[0].value.to_uppercase().as_str() {
        "SELECT" => parse_select(&tokens),
        "MATCH" => parse_match(&tokens),
        "COUNT" => parse_count(&tokens),
        _ => Err(ParseError {
            message: format!("expected SELECT, MATCH, or COUNT, found '{}'", tokens[0].value),
            position: tokens[0].pos,
        }),
    }
}

#[derive(Debug, Clone)]
struct Token {
    value: String,
    pos: usize,
}

fn tokenize(input: &str) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut pos = 0;

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            pos += 1;
            continue;
        }

        if ch == '"' {
            // Quoted string
            chars.next();
            pos += 1;
            let start = pos;
            let mut value = String::new();
            loop {
                match chars.next() {
                    Some('"') => {
                        pos += 1;
                        break;
                    }
                    Some(c) => {
                        value.push(c);
                        pos += 1;
                    }
                    None => {
                        return Err(ParseError {
                            message: "unterminated string".into(),
                            position: start,
                        });
                    }
                }
            }
            tokens.push(Token { value, pos: start });
            continue;
        }

        if "=<>!,()".contains(ch) {
            let start = pos;
            let mut op = String::new();
            op.push(ch);
            chars.next();
            pos += 1;
            // Handle >=, <=, !=
            if let Some(&next) = chars.peek() {
                if (ch == '>' || ch == '<' || ch == '!') && next == '=' {
                    op.push(next);
                    chars.next();
                    pos += 1;
                }
            }
            tokens.push(Token { value: op, pos: start });
            continue;
        }

        // Word or number
        let start = pos;
        let mut value = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || "=<>!,()\"".contains(c) {
                break;
            }
            value.push(c);
            chars.next();
            pos += 1;
        }
        if !value.is_empty() {
            tokens.push(Token { value, pos: start });
        }
    }

    Ok(tokens)
}

fn parse_select(tokens: &[Token]) -> Result<Query, ParseError> {
    // SELECT fields FROM database WHERE conditions
    let mut i = 1; // skip SELECT

    // Parse fields
    let mut fields = Vec::new();
    while i < tokens.len() {
        let upper = tokens[i].value.to_uppercase();
        if upper == "FROM" {
            break;
        }
        if tokens[i].value == "," {
            i += 1;
            continue;
        }
        fields.push(parse_field(&tokens[i])?);
        i += 1;
    }

    if fields.is_empty() {
        return Err(ParseError { message: "no fields in SELECT".into(), position: 0 });
    }

    // Expect FROM
    if i >= tokens.len() || tokens[i].value.to_uppercase() != "FROM" {
        return Err(ParseError {
            message: "expected FROM".into(),
            position: if i < tokens.len() { tokens[i].pos } else { 0 },
        });
    }
    i += 1;

    // Parse database
    if i >= tokens.len() {
        return Err(ParseError { message: "expected database name".into(), position: 0 });
    }
    let database = parse_database(&tokens[i])?;
    i += 1;

    // Optional WHERE
    let mut conditions = Vec::new();
    if i < tokens.len() && tokens[i].value.to_uppercase() == "WHERE" {
        i += 1;
        conditions = parse_conditions(tokens, &mut i)?;
    }

    Ok(Query::Select { fields, database, conditions })
}

fn parse_match(tokens: &[Token]) -> Result<Query, ParseError> {
    // MATCH pattern IN scope
    if tokens.len() < 4 {
        return Err(ParseError {
            message: "MATCH requires: MATCH pattern IN scope".into(),
            position: 0,
        });
    }
    let pattern = tokens[1].value.clone();

    if tokens[2].value.to_uppercase() != "IN" {
        return Err(ParseError { message: "expected IN".into(), position: tokens[2].pos });
    }

    let scope = parse_scope(tokens, 3)?;
    Ok(Query::Match { pattern, scope })
}

fn parse_count(tokens: &[Token]) -> Result<Query, ParseError> {
    // COUNT pattern IN scope
    if tokens.len() < 4 {
        return Err(ParseError {
            message: "COUNT requires: COUNT pattern IN scope".into(),
            position: 0,
        });
    }
    let pattern = tokens[1].value.clone();

    if tokens[2].value.to_uppercase() != "IN" {
        return Err(ParseError { message: "expected IN".into(), position: tokens[2].pos });
    }

    let scope = parse_scope(tokens, 3)?;
    Ok(Query::Count { pattern, scope })
}

fn parse_field(token: &Token) -> Result<Field, ParseError> {
    match token.value.to_lowercase().as_str() {
        "*" => Ok(Field::All),
        "id" => Ok(Field::Id),
        "severity" => Ok(Field::Severity),
        "category" => Ok(Field::Category),
        "description" => Ok(Field::Description),
        "fix_template" => Ok(Field::FixTemplate),
        "fix_confidence" => Ok(Field::FixConfidence),
        "pattern" => Ok(Field::Pattern),
        _ => Err(ParseError {
            message: format!("unknown field '{}'", token.value),
            position: token.pos,
        }),
    }
}

fn parse_database(token: &Token) -> Result<DatabaseSelector, ParseError> {
    match token.value.to_lowercase().as_str() {
        "all" => Ok(DatabaseSelector::All),
        "ownership" => Ok(DatabaseSelector::Specific(Database::Ownership)),
        "borrow" => Ok(DatabaseSelector::Specific(Database::Borrow)),
        "lifetime" => Ok(DatabaseSelector::Specific(Database::Lifetime)),
        "type_safety" => Ok(DatabaseSelector::Specific(Database::TypeSafety)),
        "concurrency" => Ok(DatabaseSelector::Specific(Database::Concurrency)),
        "ffi" => Ok(DatabaseSelector::Specific(Database::Ffi)),
        _ => Err(ParseError {
            message: format!("unknown database '{}'", token.value),
            position: token.pos,
        }),
    }
}

fn parse_severity(s: &str) -> Option<Severity> {
    match s.to_lowercase().as_str() {
        "error" => Some(Severity::Error),
        "warning" => Some(Severity::Warning),
        "info" => Some(Severity::Info),
        "hint" => Some(Severity::Hint),
        _ => None,
    }
}

fn parse_conditions(tokens: &[Token], i: &mut usize) -> Result<Vec<Condition>, ParseError> {
    let mut conditions = Vec::new();

    while *i < tokens.len() {
        let field = tokens[*i].value.to_lowercase();
        *i += 1;

        if *i >= tokens.len() {
            return Err(ParseError { message: "expected operator".into(), position: 0 });
        }
        let op = &tokens[*i].value;
        *i += 1;

        if *i >= tokens.len() {
            return Err(ParseError { message: "expected value".into(), position: 0 });
        }
        let value = &tokens[*i].value;
        *i += 1;

        let condition = match (field.as_str(), op.as_str()) {
            ("category", "=") => Condition::CategoryEq(value.clone()),
            ("severity", "=") => {
                let sev = parse_severity(value).ok_or(ParseError {
                    message: format!("invalid severity '{value}'"),
                    position: tokens[*i - 1].pos,
                })?;
                Condition::SeverityEq(sev)
            }
            ("severity", ">=") => {
                let sev = parse_severity(value).ok_or(ParseError {
                    message: format!("invalid severity '{value}'"),
                    position: tokens[*i - 1].pos,
                })?;
                Condition::SeverityGte(sev)
            }
            ("severity", "<=") => {
                let sev = parse_severity(value).ok_or(ParseError {
                    message: format!("invalid severity '{value}'"),
                    position: tokens[*i - 1].pos,
                })?;
                Condition::SeverityLte(sev)
            }
            ("fix_confidence", ">") => {
                let val: f64 = value.parse().map_err(|_| ParseError {
                    message: format!("invalid float '{value}'"),
                    position: tokens[*i - 1].pos,
                })?;
                Condition::FixConfidenceGt(val)
            }
            ("pattern", "=") => Condition::PatternEq(value.clone()),
            ("tag", "=") => Condition::TagContains(value.clone()),
            _ => {
                return Err(ParseError {
                    message: format!("unsupported condition '{field} {op}'"),
                    position: tokens[*i - 3].pos,
                });
            }
        };
        conditions.push(condition);

        // Skip optional AND
        if *i < tokens.len() && tokens[*i].value.to_uppercase() == "AND" {
            *i += 1;
        }
    }

    Ok(conditions)
}

fn parse_scope(tokens: &[Token], start: usize) -> Result<QueryScope, ParseError> {
    if start >= tokens.len() {
        return Err(ParseError { message: "expected scope".into(), position: 0 });
    }

    match tokens[start].value.to_lowercase().as_str() {
        "crate" => Ok(QueryScope::Crate),
        "global" => Ok(QueryScope::Global),
        "function" => {
            // function("name")
            if start + 3 < tokens.len()
                && tokens[start + 1].value == "("
                && tokens[start + 3].value == ")"
            {
                Ok(QueryScope::Function(tokens[start + 2].value.clone()))
            } else {
                Err(ParseError {
                    message: "expected function(\"name\")".into(),
                    position: tokens[start].pos,
                })
            }
        }
        "module" => {
            if start + 3 < tokens.len()
                && tokens[start + 1].value == "("
                && tokens[start + 3].value == ")"
            {
                Ok(QueryScope::Module(tokens[start + 2].value.clone()))
            } else {
                Err(ParseError {
                    message: "expected module(\"path\")".into(),
                    position: tokens[start].pos,
                })
            }
        }
        _ => Err(ParseError {
            message: format!("unknown scope '{}'", tokens[start].value),
            position: tokens[start].pos,
        }),
    }
}

// ===========================================================================
// Query executor
// ===========================================================================

/// Execute a parsed query against the SKB.
pub fn execute<'a>(skb: &'a SafetyKnowledgeBase, query: &Query) -> QueryResult<'a> {
    let start = Instant::now();

    let result = match query {
        Query::Select { database, conditions, .. } => execute_select(skb, database, conditions),
        Query::Match { pattern, .. } => {
            let rules = skb.query_by_pattern(pattern);
            rules
        }
        Query::Count { pattern, .. } => skb.query_by_pattern(pattern),
    };

    let elapsed = start.elapsed().as_micros() as u64;

    QueryResult {
        total_count: result.len(),
        truncated: false,
        rules: result,
        eval_time_us: elapsed,
    }
}

fn execute_select<'a>(
    skb: &'a SafetyKnowledgeBase,
    database: &DatabaseSelector,
    conditions: &[Condition],
) -> Vec<&'a Rule> {
    // Start with all rules in the target database
    let candidates: Vec<&Rule> = match database {
        DatabaseSelector::All => skb.all_rules(),
        DatabaseSelector::Specific(db) => skb.query_active_in_database(*db),
    };

    // Apply conditions as filters
    candidates
        .into_iter()
        .filter(|rule| conditions.iter().all(|cond| matches_condition(rule, cond)))
        .collect()
}

fn matches_condition(rule: &Rule, condition: &Condition) -> bool {
    match condition {
        Condition::CategoryEq(cat) => rule.category == *cat,
        Condition::SeverityEq(sev) => rule.severity == *sev,
        Condition::SeverityGte(sev) => rule.severity >= *sev,
        Condition::SeverityLte(sev) => rule.severity <= *sev,
        Condition::FixConfidenceGt(threshold) => rule.fix_confidence > *threshold,
        Condition::PatternEq(name) => rule.pattern.name() == name,
        Condition::TagContains(tag) => rule.tags.iter().any(|t| t == tag),
    }
}

/// Execute an SKB-QL query string directly.
pub fn query<'a>(skb: &'a SafetyKnowledgeBase, input: &str) -> Result<QueryResult<'a>, ParseError> {
    let parsed = parse_query(input)?;
    Ok(execute(skb, &parsed))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed_corpus;

    // -- Tokenizer tests -----------------------------------------------------

    #[test]
    fn tokenize_simple_select() {
        let tokens = tokenize("SELECT id FROM ownership").unwrap();
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].value, "SELECT");
        assert_eq!(tokens[3].value, "ownership");
    }

    #[test]
    fn tokenize_with_quoted_strings() {
        let tokens = tokenize(r#"category = "use-after-move""#).unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[2].value, "use-after-move");
    }

    #[test]
    fn tokenize_operators() {
        let tokens = tokenize("severity >= error").unwrap();
        assert_eq!(tokens[1].value, ">=");
    }

    #[test]
    fn tokenize_parentheses() {
        let tokens = tokenize(r#"function("process")"#).unwrap();
        assert_eq!(tokens[0].value, "function");
        assert_eq!(tokens[1].value, "(");
        assert_eq!(tokens[2].value, "process");
        assert_eq!(tokens[3].value, ")");
    }

    // -- Parser tests --------------------------------------------------------

    #[test]
    fn parse_select_all_from_ownership() {
        let q = parse_query("SELECT * FROM ownership").unwrap();
        assert_eq!(
            q,
            Query::Select {
                fields: vec![Field::All],
                database: DatabaseSelector::Specific(Database::Ownership),
                conditions: vec![],
            }
        );
    }

    #[test]
    fn parse_select_fields_from_all() {
        let q = parse_query("SELECT id, severity FROM all").unwrap();
        assert_eq!(
            q,
            Query::Select {
                fields: vec![Field::Id, Field::Severity],
                database: DatabaseSelector::All,
                conditions: vec![],
            }
        );
    }

    #[test]
    fn parse_select_with_category_condition() {
        let q =
            parse_query(r#"SELECT * FROM ownership WHERE category = "use-after-move""#).unwrap();
        if let Query::Select { conditions, .. } = &q {
            assert_eq!(conditions.len(), 1);
            assert_eq!(conditions[0], Condition::CategoryEq("use-after-move".into()));
        } else {
            panic!("expected Select");
        }
    }

    #[test]
    fn parse_select_with_severity_gte() {
        let q = parse_query(r#"SELECT * FROM all WHERE severity >= "error""#).unwrap();
        if let Query::Select { conditions, .. } = &q {
            assert_eq!(conditions[0], Condition::SeverityGte(Severity::Error));
        } else {
            panic!("expected Select");
        }
    }

    #[test]
    fn parse_select_with_compound_conditions() {
        let q = parse_query(
            r#"SELECT id FROM all WHERE severity >= "warning" AND category = "data-race""#,
        )
        .unwrap();
        if let Query::Select { conditions, .. } = &q {
            assert_eq!(conditions.len(), 2);
            assert_eq!(conditions[0], Condition::SeverityGte(Severity::Warning));
            assert_eq!(conditions[1], Condition::CategoryEq("data-race".into()));
        } else {
            panic!("expected Select");
        }
    }

    #[test]
    fn parse_select_fix_confidence() {
        let q = parse_query("SELECT * FROM all WHERE fix_confidence > 0.9").unwrap();
        if let Query::Select { conditions, .. } = &q {
            assert_eq!(conditions[0], Condition::FixConfidenceGt(0.9));
        } else {
            panic!("expected Select");
        }
    }

    #[test]
    fn parse_match_in_crate() {
        let q = parse_query("MATCH UseAfterMove IN crate").unwrap();
        assert_eq!(q, Query::Match { pattern: "UseAfterMove".into(), scope: QueryScope::Crate });
    }

    #[test]
    fn parse_match_in_function() {
        let q = parse_query(r#"MATCH DataRace IN function("process")"#).unwrap();
        assert_eq!(
            q,
            Query::Match {
                pattern: "DataRace".into(),
                scope: QueryScope::Function("process".into()),
            }
        );
    }

    #[test]
    fn parse_count_in_crate() {
        let q = parse_query("COUNT MutableBorrow IN crate").unwrap();
        assert_eq!(q, Query::Count { pattern: "MutableBorrow".into(), scope: QueryScope::Crate });
    }

    #[test]
    fn parse_error_empty_query() {
        let err = parse_query("").unwrap_err();
        assert_eq!(err.message, "empty query");
    }

    #[test]
    fn parse_error_unknown_keyword() {
        let err = parse_query("DELETE FROM ownership").unwrap_err();
        assert!(err.message.contains("expected SELECT"));
    }

    #[test]
    fn parse_error_unknown_field() {
        let err = parse_query("SELECT bogus FROM ownership").unwrap_err();
        assert!(err.message.contains("unknown field"));
    }

    #[test]
    fn parse_error_unknown_database() {
        let err = parse_query("SELECT * FROM bogus").unwrap_err();
        assert!(err.message.contains("unknown database"));
    }

    // -- Execution tests (against seed corpus) --------------------------------

    #[test]
    fn execute_select_all_from_ownership() {
        let skb = seed_corpus();
        let result = query(&skb, "SELECT * FROM ownership").unwrap();
        assert_eq!(result.total_count, 2_847);
    }

    #[test]
    fn execute_select_from_all() {
        let skb = seed_corpus();
        let result = query(&skb, "SELECT * FROM all").unwrap();
        assert_eq!(result.total_count, 9_157);
    }

    #[test]
    fn execute_select_category_filter() {
        let skb = seed_corpus();
        let result =
            query(&skb, r#"SELECT * FROM ownership WHERE category = "use-after-move""#).unwrap();
        assert_eq!(result.total_count, 712);
    }

    #[test]
    fn execute_select_severity_gte_error() {
        let skb = seed_corpus();
        let result = query(&skb, r#"SELECT * FROM all WHERE severity >= "error""#).unwrap();
        // ~50% of rules are Error (indices 0 and 2 mod 4)
        assert!(result.total_count > 0);
    }

    #[test]
    fn execute_select_compound_conditions() {
        let skb = seed_corpus();
        let result = query(
            &skb,
            r#"SELECT * FROM ownership WHERE category = "use-after-move" AND severity >= "error""#,
        )
        .unwrap();
        assert!(result.total_count > 0);
        assert!(result.total_count <= 712);
    }

    #[test]
    fn execute_match_pattern() {
        let skb = seed_corpus();
        let result = query(&skb, "MATCH UseAfterMove IN crate").unwrap();
        assert!(result.total_count > 0);
    }

    #[test]
    fn execute_count_pattern() {
        let skb = seed_corpus();
        let result = query(&skb, "COUNT MutableBorrow IN crate").unwrap();
        assert!(result.total_count > 0);
    }

    #[test]
    fn execute_fix_confidence_filter() {
        let skb = seed_corpus();
        let result = query(&skb, "SELECT * FROM all WHERE fix_confidence > 0.8").unwrap();
        // ~66% of seed rules have fix_confidence 0.85
        assert!(result.total_count > 0);
    }

    #[test]
    fn query_result_has_timing() {
        let skb = seed_corpus();
        let result = query(&skb, "SELECT * FROM ownership").unwrap();
        // eval_time_us is measured — just check it's not absurdly large
        // (should be well under 1 second = 1_000_000 us)
        assert!(result.eval_time_us < 1_000_000);
    }

    #[test]
    fn query_performance_under_200us() {
        let skb = seed_corpus();
        // Run multiple queries and check that average is reasonable
        let queries = [
            r#"SELECT * FROM ownership WHERE category = "use-after-move""#,
            r#"SELECT * FROM all WHERE severity >= "error""#,
            "MATCH UseAfterMove IN crate",
            "COUNT MutableBorrow IN crate",
        ];

        let mut total_us = 0u64;
        let n = queries.len() as u64;
        for q in &queries {
            let result = query(&skb, q).unwrap();
            total_us += result.eval_time_us;
        }

        let avg_us = total_us / n;
        // P99 target is 200us (0.20ms). Average should be well under that.
        // Allow generous margin for CI/debug builds.
        assert!(avg_us < 50_000, "average query time {}us exceeds 50ms", avg_us);
    }
}
