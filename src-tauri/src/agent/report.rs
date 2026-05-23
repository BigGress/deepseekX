use crate::agent::state::AgentObservation;

pub fn summarize_observations(
    observations: &[AgentObservation],
    focus: &str,
    output_format: &str,
) -> String {
    let mut lines = vec![
        format!("聚焦主题: {}", focus),
        format!("输出格式: {}", output_format),
        String::new(),
    ];

    for observation in observations.iter().rev().take(6).rev() {
        lines.push(format!(
            "- [{}] {}",
            observation.action_name, observation.summary
        ));
        if !observation.result_summary.trim().is_empty() {
            lines.push(format!("  结果: {}", observation.result_summary));
        }
        if let Some(diff) = observation.diff_preview.as_ref() {
            lines.push(format!("  Diff:\n{}", diff));
        } else if let Some(after) = observation.after_preview.as_ref() {
            lines.push(format!("  预览:\n{}", after));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::summarize_observations;
    use crate::agent::state::{AgentObservation, AgentStepStatus};

    #[test]
    fn builds_summary_from_recent_observations() {
        let summary = summarize_observations(
            &[AgentObservation {
                action_name: "write_files".into(),
                reason: "update".into(),
                input_summary: "src/App.tsx".into(),
                result_summary: "updated".into(),
                summary: "已写入 1 个文件".into(),
                status: AgentStepStatus::Completed,
                is_error: false,
                requires_confirmation: false,
                preview_type: None,
                before_preview: None,
                after_preview: Some("new code".into()),
                diff_preview: None,
                changed_ranges: None,
            }],
            "改动整理",
            "change_summary",
        );

        assert!(summary.contains("改动整理"));
        assert!(summary.contains("write_files"));
    }
}
