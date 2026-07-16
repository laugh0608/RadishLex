use radishlex_ime_userdb::{LearningCaseInspection, UserDb};

use super::{
    format_optional_ms, parse_named_options, required_named_option, validate_input_code, CliError,
};

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    let options = parse_named_options(
        args,
        3,
        &["db", "input", "text", "reading", "context"],
        "learn case-status",
    )?;
    let db_path = required_named_option(&options, "db")?;
    let input_code = required_named_option(&options, "input")?;
    validate_input_code(input_code)?;
    let text = required_named_option(&options, "text")?;
    let reading = options.get("reading").map(String::as_str);
    let context_kind = options.get("context").map_or("general", String::as_str);

    let mut db = UserDb::open(db_path)?;
    let inspection = db.inspect_learning_case(input_code, text, reading, context_kind)?;
    Ok(render(&inspection))
}

fn render(inspection: &LearningCaseInspection) -> String {
    let mut output = String::new();
    output.push_str("learning_case_status: ready\n");
    output.push_str(&format!(
        "inspection_version: {}\n",
        inspection.inspection_version
    ));
    output.push_str("identity:\n");
    output.push_str(&format!("  input: {}\n", inspection.identity.input_code));
    output.push_str(&format!("  text: {}\n", inspection.identity.text));
    output.push_str(&format!(
        "  reading: {}\n",
        inspection.identity.reading.as_deref().unwrap_or("<none>")
    ));
    output.push_str(&format!(
        "  context: {}\n",
        inspection.identity.context_kind
    ));

    let aggregate = &inspection.aggregate;
    output.push_str("aggregate:\n");
    output.push_str(&format!("  schema_version: {}\n", aggregate.schema_version));
    output.push_str(&format!(
        "  active_user_terms: {}\n",
        aggregate.active_user_terms
    ));
    output.push_str(&format!(
        "  suppressed_user_terms: {}\n",
        aggregate.suppressed_user_terms
    ));
    output.push_str(&format!("  ranker_weights: {}\n", aggregate.ranker_weights));
    output.push_str(&format!(
        "  deleted_tombstones: {}\n",
        aggregate.deleted_term_tombstones
    ));
    output.push_str(&format!(
        "  selection_events: {}\n",
        aggregate.selection_events
    ));
    output.push_str(&format!(
        "  negative_feedback: {}\n",
        aggregate.negative_feedback
    ));
    output.push_str(&format!("  import_batches: {}\n", aggregate.import_batches));
    output.push_str(&format!(
        "  user_terms_updated_at_ms: {}\n",
        format_optional_ms(aggregate.latest_user_term_updated_at_ms)
    ));
    output.push_str(&format!(
        "  selection_event_at_ms: {}\n",
        format_optional_ms(aggregate.latest_selection_event_at_ms)
    ));
    output.push_str(&format!(
        "  negative_feedback_at_ms: {}\n",
        format_optional_ms(aggregate.latest_negative_feedback_at_ms)
    ));
    output.push_str(&format!(
        "  deleted_term_at_ms: {}\n",
        format_optional_ms(aggregate.latest_deleted_term_at_ms)
    ));
    output.push_str(&format!(
        "  import_batch_at_ms: {}\n",
        format_optional_ms(aggregate.latest_import_batch_at_ms)
    ));
    output.push_str(&format!(
        "  overall_at_ms: {}\n",
        format_optional_ms(aggregate.latest_activity_at_ms)
    ));

    output.push_str("term:\n");
    if let Some(term) = &inspection.term {
        output.push_str("  present: true\n");
        output.push_str(&format!("  source: {}\n", term.source));
        output.push_str(&format!("  status: {}\n", term.status));
        output.push_str(&format!("  weight: {}\n", term.weight));
        output.push_str(&format!("  version_ms: {}\n", term.version_ms));
        output.push_str(&format!(
            "  last_used_at_ms: {}\n",
            format_optional_ms(term.last_used_at_ms)
        ));
        output.push_str(&format!(
            "  restored_at_ms: {}\n",
            format_optional_ms(term.restored_at_ms)
        ));
    } else {
        output.push_str("  present: false\n");
    }

    output.push_str("ranker_weight:\n");
    if let Some(ranker_weight) = &inspection.ranker_weight {
        output.push_str("  present: true\n");
        output.push_str(&format!("  frequency: {}\n", ranker_weight.frequency));
        output.push_str(&format!(
            "  last_used_at_ms: {}\n",
            format_optional_ms(ranker_weight.last_used_at_ms)
        ));
        output.push_str(&format!(
            "  negative_score: {}\n",
            ranker_weight.negative_score
        ));
        output.push_str(&format!(
            "  updated_at_ms: {}\n",
            ranker_weight.updated_at_ms
        ));
    } else {
        output.push_str("  present: false\n");
    }

    output.push_str("deleted_tombstone:\n");
    if let Some(tombstone) = inspection.deleted_tombstone {
        output.push_str("  present: true\n");
        output.push_str(&format!("  deleted_at_ms: {}\n", tombstone.deleted_at_ms));
    } else {
        output.push_str("  present: false\n");
    }
    output.push_str("p1_rows: omitted\n");
    output
}
