pub fn is_valid_version(version: &str) -> bool {
    hl7_definitions::VERSIONS.contains(&version)
}

pub fn segment_description(version: &str, segment: &str) -> String {
    hl7_definitions::get_segment(version, segment)
        .map(|s| s.description.to_string())
        .unwrap_or_else(|| "Unknown segment".to_string())
}

pub fn describe_field(version: &str, segment: &str, field: usize) -> String {
    hl7_definitions::get_segment(version, segment)
        .map(|s| {
            s.fields
                .get(field - 1)
                .map(|f| {
                    let datatype = hl7_definitions::get_field(version, f.datatype)
                        .map(|d| d.description)
                        .unwrap_or_else(|| "Unknown datatype");

                    let repeat = match f.repeatability {
                        hl7_definitions::FieldRepeatability::Unbounded => "∞",
                        hl7_definitions::FieldRepeatability::Single => "1",
                        hl7_definitions::FieldRepeatability::Bounded(n) => &n.to_string(),
                    };

                    let optional = match f.optionality {
                        hl7_definitions::FieldOptionality::Required => "*required*",
                        hl7_definitions::FieldOptionality::Optional => "*optional*",
                        hl7_definitions::FieldOptionality::Conditional => "*conditional*",
                        hl7_definitions::FieldOptionality::BackwardCompatibility => {
                            "*backwards compatibility*"
                        }
                    };

                    let table = f.table.map(|t| {
                        hl7_definitions::table_values(t as u16)
                            .map(|values| {
                                let mut values = values
                                    .iter()
                                    .map(|(code, description)| {
                                        format!("    `{code}` ({description})")
                                    })
                                    .collect::<Vec<String>>();
                                values.sort();
                                values.join("\n")
                            })
                            .unwrap_or_default()
                    });
                    let table = table
                        .map(|t| format!("\n  Table values:\n{}", t))
                        .unwrap_or_default();

                    format!(
                        "{description}, len: {len} ({datatype}) [{optional}/{repeat}]{table}",
                        description = f.description,
                        len = f
                            .max_length
                            .map(|l| l.to_string())
                            .unwrap_or_else(|| "∞".to_string()),
                    )
                })
                .unwrap_or_else(|| "Unknown field".to_string())
        })
        .unwrap_or_else(|| "Unknown segment".to_string())
}

pub fn describe_component(_version: &str, _segment: &str, _field: usize, _component: usize) -> String {
    "Not implemented yet in the LSP! glhfdd".to_string()
}
