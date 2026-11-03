//! GST state codes, for showing which state a code means and listing them to pick from

const STATES: &[(&str, &str)] = &[
    ("01", "Jammu and Kashmir"),
    ("02", "Himachal Pradesh"),
    ("03", "Punjab"),
    ("04", "Chandigarh"),
    ("05", "Uttarakhand"),
    ("06", "Haryana"),
    ("07", "Delhi"),
    ("08", "Rajasthan"),
    ("09", "Uttar Pradesh"),
    ("10", "Bihar"),
    ("11", "Sikkim"),
    ("12", "Arunachal Pradesh"),
    ("13", "Nagaland"),
    ("14", "Manipur"),
    ("15", "Mizoram"),
    ("16", "Tripura"),
    ("17", "Meghalaya"),
    ("18", "Assam"),
    ("19", "West Bengal"),
    ("20", "Jharkhand"),
    ("21", "Odisha"),
    ("22", "Chhattisgarh"),
    ("23", "Madhya Pradesh"),
    ("24", "Gujarat"),
    ("26", "Dadra and Nagar Haveli and Daman and Diu"),
    ("27", "Maharashtra"),
    ("29", "Karnataka"),
    ("30", "Goa"),
    ("31", "Lakshadweep"),
    ("32", "Kerala"),
    ("33", "Tamil Nadu"),
    ("34", "Puducherry"),
    ("35", "Andaman and Nicobar Islands"),
    ("36", "Telangana"),
    ("37", "Andhra Pradesh"),
    ("38", "Ladakh"),
    ("97", "Other Territory"),
];

/// `Some("Karnataka")` for `"29"`
pub fn state_name(code: &str) -> Option<&'static str> {
    STATES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| *name)
}

/// A GST state or union territory: what a client in India picks as their place of supply.
/// Front ends list these rather than format codes and names themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GstState {
    code: &'static str,
    name: &'static str,
}

impl GstState {
    /// `29`, as a client's state is stored and typed
    pub fn code(self) -> &'static str {
        self.code
    }

    /// `Karnataka`
    pub fn name(self) -> &'static str {
        self.name
    }

    /// `Karnataka (29)`, as every list of states shows one
    pub fn label(self) -> String {
        format!("{} ({})", self.name, self.code)
    }
}

/// `Karnataka (29)`, or the bare code when it isn't a GST state code
pub fn state_label(code: &str) -> String {
    states()
        .find(|state| state.code == code)
        .map_or_else(|| code.to_owned(), GstState::label)
}

/// Every GST state and union territory, in code order: the choices for a client's place of
/// supply
pub fn states() -> impl Iterator<Item = GstState> {
    STATES.iter().map(|&(code, name)| GstState { code, name })
}
