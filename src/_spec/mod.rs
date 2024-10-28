use phf::phf_map;

pub mod data_types;

pub static HL7_SEGMENT_DESCRIPTION: phf::Map<&'static str, &'static str> = phf_map! {
    "MSH" => "Message Header",
    "EVN" => "Event Type",
    "PID" => "Patient Identification",
    "PV1" => "Patient Visit",
    "PV2" => "Patient Visit - Additional Information",
    "IN1" => "Insurance",
    "IN2" => "Insurance Additional Information",
    "IN3" => "Insurance Additional Information, Certification",
    "GT1" => "Guarantor",
    "NK1" => "Next of Kin / Associated Parties",
    "AL1" => "Patient Allergy Information",
    "DG1" => "Diagnosis",
    "DRG" => "Diagnosis Related Group",
    "PR1" => "Procedures",
    "ROL" => "Role",
    "OBX" => "Observation/Result",
    "NTE" => "Notes and Comments",
    "MSA" => "Message Acknowledgment",
};

pub static HL7_FIELD_DESCRIPTION: phf::Map<&'static str, phf::Map<u32, &'static str>> = phf_map! {
    "MSH" => phf_map! {
        1u32 => "Field Separator",
        2u32 => "Encoding Characters",
        3u32 => "Sending Application",
        4u32 => "Sending Facility",
        5u32 => "Receiving Application",
        6u32 => "Receiving Facility",
        7u32 => "Date/Time of Message",
        8u32 => "Security",
        9u32 => "Message Type",
        10u32 => "Message Control ID",
        11u32 => "Processing ID",
        12u32 => "Version ID",
        13u32 => "Sequence Number",
        14u32 => "Continuation Pointer",
        15u32 => "Accept Acknowledgment Type",
        16u32 => "Application Acknowledgment Type",
        17u32 => "Country Code",
        18u32 => "Character Set",
        19u32 => "Principal Language of Message",
        20u32 => "Alternate Character Set Handling Scheme",
        21u32 => "Message Profile Identifier",
    },
    "EVN"=> phf_map! {
        1u32 => "Set ID - EVN",
        2u32 => "Recorded Date/Time",
        3u32 => "Date/Time Planned Event",
        4u32 => "Event Reason Code",
        5u32 => "Operator ID",
        6u32 => "Event Occurred",
        7u32 => "Event Facility",
    },
    "PID"=> phf_map! {
        1u32 => "Set ID - PID",
        2u32 => "Patient ID",
        3u32 => "Patient Identifier List",
        4u32 => "Alternate Patient ID - PID",
        5u32 => "Patient Name",
        6u32 => "Mother's Maiden Name",
        7u32 => "Date/Time of Birth",
        8u32 => "Administrative Sex",
        9u32 => "Patient Alias",
        10u32 => "Race",
        11u32 => "Patient Address",
        12u32 => "County Code",
        13u32 => "Phone Number - Home",
        14u32 => "Phone Number - Business",
        15u32 => "Primary Language",
        16u32 => "Marital Status",
        17u32 => "Religion",
        18u32 => "Patient Account Number",
        19u32 => "SSN Number - Patient",
        20u32 => "Driver's License Number - Patient",
        21u32 => "Mother's Identifier",
        22u32 => "Ethnic Group",
        23u32 => "Birth Place",
        24u32 => "Multiple Birth Indicator",
        25u32 => "Birth Order",
        26u32 => "Citizenship",
        27u32 => "Veterans Military Status",
        28u32 => "Nationality",
        29u32 => "Patient Death Date and Time",
        30u32 => "Patient Death Indicator",
        31u32 => "Identity Unknown Indicator",
        32u32 => "Identity Reliability Code",
        33u32 => "Last Update Date/Time",
        34u32 => "Last Update Facility",
        35u32 => "Species Code",
        36u32 => "Breed Code",
        37u32 => "Strain",
        38u32 => "Production Class Code",
        39u32 => "Tribal Citizenship",
    },
    "PV1" => phf_map! {
        1u32 => "Set ID - PV1",
        2u32 => "Patient Class",
        3u32 => "Assigned Patient Location",
        4u32 => "Admission Type",
        5u32 => "Preadmit Number",
        6u32 => "Prior Patient Location",
        7u32 => "Attending Doctor",
        8u32 => "Referring Doctor",
        9u32 => "Consulting Doctor",
        10u32 => "Hospital Service",
        11u32 => "Temporary Location",
        12u32 => "Preadmit Test Indicator",
        13u32 => "Re-admission Indicator",
        14u32 => "Admit Source",
        15u32 => "Ambulatory Status",
        16u32 => "VIP Indicator",
        17u32 => "Admitting Doctor",
        18u32 => "Patient Type",
        19u32 => "Visit Number",
        20u32 => "Financial Class",
        21u32 => "Charge Price Indicator",
        22u32 => "Courtesy Code",
        23u32 => "Credit Rating",
        24u32 => "Contract Code",
        25u32 => "Contract Effective Date",
        26u32 => "Contract Amount",
        27u32 => "Contract Period",
        28u32 => "Interest Code",
        29u32 => "Transfer to Bad Debt Code",
        30u32 => "Transfer to Bad Debt Date",
        31u32 => "Bad Debt Agency Code",
        32u32 => "Bad Debt Transfer Amount",
        33u32 => "Bad Debt Recovery Amount",
        34u32 => "Delete Account Indicator",
        35u32 => "Delete Account Date",
        36u32 => "Discharge Disposition",
        37u32 => "Discharged to Location",
        38u32 => "Diet Type",
        39u32 => "Servicing Facility",
        40u32 => "Bed Status",
        41u32 => "Account Status",
        42u32 => "Pending Location",
        43u32 => "Prior Temporary Location",
        44u32 => "Admit Date/Time",
        45u32 => "Discharge Date/Time",
        46u32 => "Current Patient Balance",
        47u32 => "Total Charges",
        48u32 => "Total Adjustments",
        49u32 => "Total Payments",
        50u32 => "Alternate Visit ID",
        51u32 => "Visit Indicator",
        52u32 => "Other Healthcare Provider",
    }
};