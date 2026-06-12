// ── Example 1: Swarm Audit ──────────────────────────────────────────
//
// Demonstrates multi-agent audit workflow using the Swarm Bus:
// 1. A producer agent publishes compilation results
// 2. An auditor agent checks contracts and effects
// 3. Results are aggregated into a report

// ── Types ──────────────────────────────────────────────────────────

+S CompilationUnit {
    name: s,
    source: s,
    contracts: [s]~,
    effects: [s]~,
}

+S AuditFinding {
    unit_name: s,
    severity: AuditSeverity,
    message: s,
}

+E AuditSeverity {
    Info,
    Warning,
    Error,
}

+S AuditReport {
    findings: [AuditFinding]~,
    units_audited: u32,
    pass_rate: f64,
}

// ── Producer Agent ─────────────────────────────────────────────────

// Agent A: Discovers compilation units and publishes them for audit.
//
// +f discover_units(project_root: &s) -> [CompilationUnit]~
//     @fx fs, io
// {
//     val files = std.fs.read_dir(project_root)?;
//     var units = Vec.new();
//     @ entry in files {
//         val source = std.fs.read_to_string(entry.path())?;
//         val contracts = extract_contracts(&source);
//         val effects = extract_effects(&source);
//         units.push(CompilationUnit {
//             name: entry.file_name().to_string(),
//             source,
//             contracts,
//             effects,
//         });
//     }
//     units
// }

// ── Auditor Agent ──────────────────────────────────────────────────

// Agent B: Receives compilation units and checks for issues.
//
// f audit_unit(unit: &CompilationUnit) -> [AuditFinding]~
//     @fx pure
// {
//     var findings = Vec.new();
//
//     // Check: all public functions should have contracts
//     ?: unit.contracts.is_empty() {
//         findings.push(AuditFinding {
//             unit_name: unit.name.clone(),
//             severity: AuditSeverity.Warning,
//             message: "No contracts found".into(),
//         });
//     }
//
//     // Check: IO effects should be declared
//     ?: unit.source.contains("read_to_string") && !unit.effects.contains(&"io".into()) {
//         findings.push(AuditFinding {
//             unit_name: unit.name.clone(),
//             severity: AuditSeverity.Error,
//             message: "IO operation without @fx io annotation".into(),
//         });
//     }
//
//     // Check: unsafe code should have explicit effects
//     ?: unit.source.contains("unsafe") && !unit.effects.contains(&"unsafe".into()) {
//         findings.push(AuditFinding {
//             unit_name: unit.name.clone(),
//             severity: AuditSeverity.Error,
//             message: "Unsafe code without @fx unsafe annotation".into(),
//         });
//     }
//
//     findings
// }

// ── Aggregator ─────────────────────────────────────────────────────

// Agent C: Collects all findings and produces the report.
//
// f aggregate(all_findings: [([AuditFinding]~)]~, total_units: u32) -> AuditReport
//     @fx pure
// {
//     val findings: [AuditFinding]~ = all_findings.into_iter().flatten().collect();
//     val error_count = findings.iter()
//         .filter(|f| matches!(f.severity, AuditSeverity.Error))
//         .count() as u32;
//     val pass_rate = ?: total_units > 0 {
//         (total_units - error_count) as f64 / total_units as f64
//     } _ { 1.0 };
//
//     AuditReport { findings, units_audited: total_units, pass_rate }
// }

// ── Swarm Orchestration ───────────────────────────────────────────

// +af run_swarm_audit(project_root: &s) -> AuditReport
//     @fx io, fs, async
// {
//     val bus = SwarmBus.new();
//
//     // Phase 1: Discovery
//     val units = discover_units(project_root);
//     @ unit in &units {
//         bus.publish("audit.unit", unit.clone());
//     }
//
//     // Phase 2: Audit (parallel agents)
//     var all_findings = Vec.new();
//     @ unit in &units {
//         val findings = audit_unit(unit);
//         all_findings.push(findings);
//         bus.publish("audit.finding", findings.clone());
//     }
//
//     // Phase 3: Aggregate
//     val report = aggregate(all_findings, units.len() as u32);
//     bus.publish("audit.report", report.clone());
//
//     p"Audit complete: {report.units_audited} units, {report.pass_rate:.0%} pass rate";
//     report
// }
