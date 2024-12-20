pub fn is_valid_version(version: &str) -> bool {
    hl7_definitions::VERSIONS.contains(&version)
}

pub fn segment_description(version: &str, segment: &str) -> String {
    hl7_definitions::get_segment(version, segment)
        .map(|s| s.description.to_string())
        .unwrap_or_else(|| "Unknown segment".to_string())
}

pub fn is_field_a_timestamp(version: &str, segment: &str, field: usize) -> bool {
    hl7_definitions::get_segment(version, segment)
        .and_then(|s| s.fields.get(field - 1))
        .map(|f| f.datatype == "TS" || f.datatype == "DTM")
        .unwrap_or(false)
}

pub fn is_component_a_timestamp(
    version: &str,
    segment: &str,
    field: usize,
    component: usize,
) -> bool {
    hl7_definitions::get_segment(version, segment)
        .and_then(|s| s.fields.get(field - 1))
        .and_then(|f| hl7_definitions::get_field(version, f.datatype))
        .and_then(|f| f.subfields.get(component - 1))
        .map(|c| c.datatype == "TS" || c.datatype == "DTM")
        .unwrap_or(false)
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
                                        format!("      `{code}` ({description})")
                                    })
                                    .collect::<Vec<String>>();
                                values.sort();
                                values.join("\n")
                            })
                            .unwrap_or_default()
                    });
                    let table = table
                        .map(|t| format!("\n    Table values:\n{t}"))
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

pub fn describe_component(version: &str, segment: &str, field: usize, component: usize) -> String {
    hl7_definitions::get_segment(version, segment)
        .map(|s| {
            s.fields
                .get(field - 1)
                .map(|f| {
                    hl7_definitions::get_field(version, f.datatype)
                        .and_then(|f| f.subfields.get(component - 1))
                        .map(|c| {
                            let datatype = hl7_definitions::get_field(version, c.datatype)
                                .map(|d| d.description)
                                .unwrap_or_else(|| "Unknown datatype");

                            let repeat = match c.repeatability {
                                hl7_definitions::FieldRepeatability::Unbounded => "∞",
                                hl7_definitions::FieldRepeatability::Single => "1",
                                hl7_definitions::FieldRepeatability::Bounded(n) => &n.to_string(),
                            };

                            let optional = match c.optionality {
                                hl7_definitions::FieldOptionality::Required => "*required*",
                                hl7_definitions::FieldOptionality::Optional => "*optional*",
                                hl7_definitions::FieldOptionality::Conditional => "*conditional*",
                                hl7_definitions::FieldOptionality::BackwardCompatibility => {
                                    "*backwards compatibility*"
                                }
                            };

                            let table = c.table.map(|t| {
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
                                description = c.description,
                                len = c
                                    .max_length
                                    .map(|l| l.to_string())
                                    .unwrap_or_else(|| "∞".to_string()),
                            )
                        })
                        .unwrap_or_else(|| "Unknown component".to_string())
                })
                .unwrap_or_else(|| "Unknown field".to_string())
        })
        .unwrap_or_else(|| "Unknown segment".to_string())
}

pub fn field_table_values(
    version: &str,
    segment: &str,
    field: usize,
) -> Option<Vec<(String, Option<String>)>> {
    // special case for version strings
    if segment == "MSH" && field == 12 {
        let mut versions: Vec<String> = hl7_definitions::VERSIONS
            .iter()
            .map(|v| v.to_string())
            .collect();
        versions.sort_by(|a, b| {
            let mut a = a
                .split('.')
                .map(|s| s.parse::<u32>().unwrap_or(0))
                .collect::<Vec<u32>>();
            if a.len() == 2 {
                a.push(0);
            }
            let mut b = b
                .split('.')
                .map(|s| s.parse::<u32>().unwrap_or(0))
                .collect::<Vec<u32>>();
            if b.len() == 2 {
                b.push(0);
            }
            a.cmp(&b)
        });
        return Some(versions.into_iter().map(|v| (v, None)).collect());
    }

    if field == 0 {
        return None;
    }

    hl7_definitions::get_segment(version, segment)
        .and_then(|s| s.fields.get(field - 1))
        .and_then(|f| f.table)
        .and_then(|t| hl7_definitions::table_values(t as u16))
        .map(|values| {
            let mut values = values
                .iter()
                .map(|(code, description)| (code.to_string(), Some(description.to_string())))
                .collect::<Vec<(String, Option<String>)>>();
            values.sort();
            values
        })
}

pub fn component_table_values(
    version: &str,
    segment: &str,
    field: usize,
    component: usize,
) -> Option<Vec<(String, Option<String>)>> {
    hl7_definitions::get_segment(version, segment)
        .and_then(|s| s.fields.get(field))
        .and_then(|f| hl7_definitions::get_field(version, f.datatype))
        .and_then(|f| f.subfields.get(component))
        .and_then(|c| c.table)
        .and_then(|t| hl7_definitions::table_values(t as u16))
        .map(|values| {
            let mut values = values
                .iter()
                .map(|(code, description)| (code.to_string(), Some(description.to_string())))
                .collect::<Vec<(String, Option<String>)>>();
            values.sort();
            values
        })
}

pub fn segment_parameters(version: &str, segment: &str) -> Option<Vec<String>> {
    hl7_definitions::get_segment(version, segment).map(|s| {
        s.fields
            .iter()
            .map(|f| {
                let required = match f.optionality {
                    hl7_definitions::FieldOptionality::Required => "*",
                    hl7_definitions::FieldOptionality::Optional => "",
                    hl7_definitions::FieldOptionality::Conditional => "?",
                    hl7_definitions::FieldOptionality::BackwardCompatibility => "!",
                };
                format!(
                    "{required}{description} ({datatype})",
                    description = f.description,
                    datatype = hl7_definitions::get_field(version, f.datatype)
                        .map(|d| d.description)
                        .unwrap_or_else(|| f.datatype)
                )
            })
            .collect()
    })
}

pub fn field_parameters(version: &str, segment: &str, field: usize) -> Option<Vec<String>> {
    hl7_definitions::get_segment(version, segment)
        .and_then(|s| s.fields.get(field - 1))
        .map(|f| {
            hl7_definitions::get_field(version, f.datatype)
                .map(|d| {
                    d.subfields
                        .iter()
                        .map(|c| {
                            let required = match c.optionality {
                                hl7_definitions::FieldOptionality::Required => "*",
                                hl7_definitions::FieldOptionality::Optional => "",
                                hl7_definitions::FieldOptionality::Conditional => "?",
                                hl7_definitions::FieldOptionality::BackwardCompatibility => "!",
                            };
                            format!(
                                "{required}{description} ({datatype})",
                                description = c.description,
                                datatype = hl7_definitions::get_field(version, c.datatype)
                                    .map(|d| d.description)
                                    .unwrap_or_else(|| c.datatype)
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
}
