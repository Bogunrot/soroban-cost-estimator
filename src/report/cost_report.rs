use comfy_table::Table;

use crate::report::fee_calc::FeeBreakdown;

/// Resource limits fetched from the network config.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub tx_max_instructions: u64,
    pub tx_memory_limit: u32,
    pub tx_max_disk_read_entries: u32,
    pub tx_max_write_ledger_entries: u32,
    pub tx_max_disk_read_bytes: u32,
    pub tx_max_write_bytes: u32,
    pub tx_max_size_bytes: u32,
}

/// A single resource warning when usage approaches a network limit.
#[derive(Debug, Clone)]
pub struct ResourceWarning {
    pub resource: &'static str,
    pub used: u64,
    pub limit: u64,
    pub percentage: f64,
}

impl std::fmt::Display for ResourceWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "  ⚠️  {} at {:.0}% ({}/{})",
            self.resource, self.percentage, self.used, self.limit
        )
    }
}

/// Default warning threshold: warn when usage exceeds 80% of the limit.
const WARNING_THRESHOLD: f64 = 0.80;

/// Checks resource usage against network limits and returns warnings for
/// resources that exceed the threshold.
///
/// Returns an empty `Vec` when all resources are below the threshold,
/// or when limits are not available (e.g. config fetch failed).
pub fn check_resource_limits(
    cpu_instructions: u64,
    memory_bytes: u64,
    read_entries: u32,
    write_entries: u32,
    read_bytes: u32,
    write_bytes: u32,
    tx_size: u32,
    limits: &ResourceLimits,
) -> Vec<ResourceWarning> {
    let mut warnings = Vec::new();

    let check = |
        warnings: &mut Vec<ResourceWarning>,
        resource: &'static str,
        used: u64,
        limit: u64,
    | {
        if limit > 0 {
            let percentage = (used as f64 / limit as f64) * 100.0;
            if percentage >= WARNING_THRESHOLD * 100.0 {
                warnings.push(ResourceWarning {
                    resource,
                    used,
                    limit,
                    percentage,
                });
            }
        }
    };

    check(&mut warnings, "CPU instructions", cpu_instructions, limits.tx_max_instructions);
    check(&mut warnings, "Memory bytes", memory_bytes, limits.tx_memory_limit as u64);
    check(&mut warnings, "Read entries", read_entries as u64, limits.tx_max_disk_read_entries as u64);
    check(&mut warnings, "Write entries", write_entries as u64, limits.tx_max_write_ledger_entries as u64);
    check(&mut warnings, "Read bytes", read_bytes as u64, limits.tx_max_disk_read_bytes as u64);
    check(&mut warnings, "Write bytes", write_bytes as u64, limits.tx_max_write_bytes as u64);
    check(&mut warnings, "Transaction size", tx_size as u64, limits.tx_max_size_bytes as u64);

    warnings
}

/// Formats resource warnings for display.
pub fn format_resource_warnings(warnings: &[ResourceWarning]) -> String {
    if warnings.is_empty() {
        return String::new();
    }
    let mut output = String::from("\n⚠️  Resource limit warnings:\n");
    for w in warnings {
        output.push_str(&format!("{w}\n"));
    }
    output.push_str("\n");
    output
}

/// Compute what percentage `part` is of `total`.
///
/// Returns a formatted string like `"29.3%"`. Returns `"0.0%"` when the
/// total is zero to avoid division by zero.
pub fn fee_percentage(part: i64, total: i64) -> String {
    if total == 0 {
        "0.0%".to_string()
    } else {
        let pct = (part as f64 / total as f64) * 100.0;
        format!("{pct:.1}%")
    }
}

/// A complete cost report for a single contract invocation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CostReport {
    /// Name of the contract function that was simulated.
    pub function: String,
    /// WASM bytes SHA-256 hash (hex).
    pub wasm_hash: String,
    /// CPU instructions consumed.
    pub cpu_instructions: u64,
    /// Memory bytes used.
    pub memory_bytes: u64,
    /// Transaction size in bytes.
    pub tx_size: u32,
    /// Number of ledger read entries.
    pub read_entries: u32,
    /// Number of ledger write entries.
    pub write_entries: u32,
    /// Number of ledger read bytes.
    pub read_bytes: u32,
    /// Number of ledger write bytes.
    pub write_bytes: u32,
    /// Fee breakdown.
    pub fee: FeeBreakdown,
    /// The ledger sequence the simulation ran against.
    pub ledger: u32,
    /// Network the simulation ran on.
    pub network: String,
}

/// Formats a cost report as a human-readable table.
pub fn format_report_table(report: &CostReport) -> String {
    let mut output = String::new();

    output.push_str(&format!("Function: {}\n", report.function));
    output.push_str(&format!(
        "Network: {} (ledger {})\n",
        report.network, report.ledger
    ));
    output.push_str(&format!("WASM hash: {}\n\n", report.wasm_hash));

    let mut table = Table::new();

    table.set_header(vec!["Resource", "Consumed", "Fee (stroops)"]);

    table.add_row(vec![
        "CPU Instructions",
        &report.cpu_instructions.to_string(),
        "", // fee is itemized in the breakdown below
    ]);
    table.add_row(vec!["Memory Bytes", &report.memory_bytes.to_string(), ""]);
    table.add_row(vec!["Read Entries", &report.read_entries.to_string(), ""]);
    table.add_row(vec!["Write Entries", &report.write_entries.to_string(), ""]);
    table.add_row(vec!["Read Bytes", &report.read_bytes.to_string(), ""]);
    table.add_row(vec!["Write Bytes", &report.write_bytes.to_string(), ""]);
    table.add_row(vec!["Transaction Size", &report.tx_size.to_string(), ""]);

    output.push_str(&table.to_string());
    output.push('\n');

    output.push_str(&format!("\nFee Breakdown:\n"));
    let total = report.fee.total_stroops;
    output.push_str(&format!(
        "  Non-refundable: {} stroops ({})\n",
        report.fee.non_refundable_stroops,
        fee_percentage(report.fee.non_refundable_stroops, total),
    ));
    output.push_str(&format!(
        "  Refundable:     {} stroops ({})\n",
        report.fee.refundable_stroops,
        fee_percentage(report.fee.refundable_stroops, total),
    ));
    output.push_str(&format!("\n  Components (of non-refundable):\n"));
    output.push_str(&format!(
        "    CPU:        {} stroops ({})\n",
        report.fee.cpu_fee_stroops,
        fee_percentage(report.fee.cpu_fee_stroops, total),
    ));
    output.push_str(&format!(
        "    Storage:    {} stroops ({})\n",
        report.fee.storage_fee_stroops,
        fee_percentage(report.fee.storage_fee_stroops, total),
    ));
    output.push_str(&format!(
        "    Bandwidth:  {} stroops ({})\n",
        report.fee.bandwidth_fee_stroops,
        fee_percentage(report.fee.bandwidth_fee_stroops, total),
    ));
    output.push_str(&format!(
        "\n  Total:          {} stroops ({})\n",
        report.fee.total_stroops, report.fee.total_xlm,
    ));

    output
}

/// Formats a cost report as a JSON string.
pub fn format_report_json(report: &CostReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_limits() -> ResourceLimits {
        ResourceLimits {
            tx_max_instructions: 1_000_000,
            tx_memory_limit: 41_943_040,
            tx_max_disk_read_entries: 100,
            tx_max_write_ledger_entries: 100,
            tx_max_disk_read_bytes: 1_000_000,
            tx_max_write_bytes: 1_000_000,
            tx_max_size_bytes: 100_000,
        }
    }

    #[test]
    fn test_no_warnings_when_below_threshold() {
        let limits = sample_limits();
        let warnings = check_resource_limits(100, 100, 10, 10, 1000, 1000, 100, &limits);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_cpu_warning_when_above_threshold() {
        let limits = sample_limits();
        // 90% of 1_000_000 = 900_000
        let warnings = check_resource_limits(900_000, 100, 10, 10, 1000, 1000, 100, &limits);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].resource, "CPU instructions");
        assert!(warnings[0].percentage >= 80.0);
    }

    #[test]
    fn test_memory_warning_when_above_threshold() {
        let limits = sample_limits();
        // 90% of 41_943_040
        let used = (41_943_040.0 * 0.90) as u64;
        let warnings = check_resource_limits(100, used, 10, 10, 1000, 1000, 100, &limits);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].resource, "Memory bytes");
    }

    #[test]
    fn test_multiple_warnings() {
        let limits = sample_limits();
        // CPU at 90%, write entries at 90%
        let warnings = check_resource_limits(900_000, 100, 10, 90, 1000, 1000, 100, &limits);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn test_zero_limit_skips_check() {
        let limits = ResourceLimits {
            tx_max_instructions: 0,
            tx_memory_limit: 41_943_040,
            ..sample_limits()
        };
        // CPU limit is 0, so no warning even at high usage
        let warnings = check_resource_limits(999_999, 100, 10, 10, 1000, 1000, 100, &limits);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_exactly_at_threshold_warns() {
        let limits = sample_limits();
        // Exactly 80% of 1_000_000
        let warnings = check_resource_limits(800_000, 100, 10, 10, 1000, 1000, 100, &limits);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_format_warnings_empty() {
        let output = format_resource_warnings(&[]);
        assert!(output.is_empty());
    }

    #[test]
    fn test_format_warnings_contains_resource() {
        let warnings = vec![ResourceWarning {
            resource: "CPU instructions",
            used: 900_000,
            limit: 1_000_000,
            percentage: 90.0,
        }];
        let output = format_resource_warnings(&warnings);
        assert!(output.contains("CPU instructions"));
        assert!(output.contains("90%"));
    }

    #[test]
    fn test_fee_percentage_normal() {
        assert_eq!(fee_percentage(50, 100), "50.0%");
        assert_eq!(fee_percentage(1, 3), "33.3%");
        assert_eq!(fee_percentage(0, 100), "0.0%");
    }

    #[test]
    fn test_fee_percentage_zero_total() {
        assert_eq!(fee_percentage(0, 0), "0.0%");
        assert_eq!(fee_percentage(100, 0), "0.0%");
    }

    #[test]
    fn test_fee_percentage_rounding() {
        assert_eq!(fee_percentage(1, 10), "10.0%");
        assert_eq!(fee_percentage(1, 3), "33.3%");
        assert_eq!(fee_percentage(2, 3), "66.7%");
    }
}
