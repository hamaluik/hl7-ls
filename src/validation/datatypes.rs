use super::{ValidationCode, ValidationError};
use hl7_parser::Message;
use lsp_types::DiagnosticSeverity;
use std::ops::Range;
use tracing::instrument;

#[instrument(level = "debug", skip(message))]
pub fn validate_message(message: &Message, version: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for segment in message.segments() {
        if let Some(segment_definition) = hl7_definitions::get_segment(version, segment.name) {
            for (fi, field) in segment.fields().enumerate() {
                if field.is_empty() {
                    continue;
                }
                for repeat in field.repeats() {
                    if repeat.is_empty() {
                        continue;
                    }
                    if let Some(field_definition) = segment_definition.fields.get(fi) {
                        match field_definition.datatype {
                            "NM" => check_numeric(repeat.raw_value(), &repeat.range, &mut errors),
                            "TS" | "DTM" => {
                                check_timestamp(repeat.raw_value(), &repeat.range, &mut errors)
                            }
                            "DT" => check_date(repeat.raw_value(), &repeat.range, &mut errors),
                            "TM" => check_time(repeat.raw_value(), &repeat.range, &mut errors),
                            _ => {
                                for (ci, component) in repeat.components().enumerate() {
                                    if component.is_empty() {
                                        continue;
                                    }
                                    let field_datatype = field_definition.datatype;
                                    if let Some(component_definition) =
                                        hl7_definitions::get_field(version, field_datatype)
                                            .and_then(|f| f.subfields.get(ci))
                                    {
                                        match component_definition.datatype {
                                            "NM" => {
                                                check_numeric(
                                                    component.raw_value(),
                                                    &component.range,
                                                    &mut errors,
                                                );
                                            }
                                            "TS" | "DTM" => check_timestamp(
                                                repeat.raw_value(),
                                                &repeat.range,
                                                &mut errors,
                                            ),
                                            "DT" => check_date(
                                                repeat.raw_value(),
                                                &repeat.range,
                                                &mut errors,
                                            ),
                                            "TM" => check_time(
                                                repeat.raw_value(),
                                                &repeat.range,
                                                &mut errors,
                                            ),
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    errors
}

fn check_numeric(value: &str, range: &Range<usize>, errors: &mut Vec<ValidationError>) {
    if value.parse::<f64>().is_err() {
        errors.push(ValidationError::new(
            ValidationCode::InvalidDataType("not a number"),
            format!("Invalid numeric value: {value}"),
            range.clone(),
            DiagnosticSeverity::WARNING,
        ));
    }
}

fn check_timestamp(value: &str, range: &Range<usize>, errors: &mut Vec<ValidationError>) {
    if let Err(e) = hl7_parser::datetime::parse_timestamp(value, false) {
        errors.push(ValidationError::new(
            ValidationCode::InvalidTimestamp,
            format!("Invalid timestamp: {e:#}"),
            range.clone(),
            DiagnosticSeverity::WARNING,
        ));
    }
}

fn check_date(value: &str, range: &Range<usize>, errors: &mut Vec<ValidationError>) {
    if let Err(e) = hl7_parser::datetime::parse_date(value, false) {
        errors.push(ValidationError::new(
            ValidationCode::InvalidTimestamp,
            format!("Invalid date: {e:#}"),
            range.clone(),
            DiagnosticSeverity::WARNING,
        ));
    }
}

fn check_time(value: &str, range: &Range<usize>, errors: &mut Vec<ValidationError>) {
    if let Err(e) = hl7_parser::datetime::parse_time(value, false) {
        errors.push(ValidationError::new(
            ValidationCode::InvalidTimestamp,
            format!("Invalid time: {e:#}"),
            range.clone(),
            DiagnosticSeverity::WARNING,
        ));
    }
}
