//! Case- and punctuation-insensitive name matching for the `FromStr`
//! implementations of the crate's vocabularies, the same rule the
//! Python bindings apply: `"modified following"`, `"Modified-Following"`
//! and `"ModifiedFollowing"` all name one convention.

/// `true` iff `input`, ignoring case and every character that is not an
/// ASCII letter or digit, spells `name` (itself letters and digits).
pub(crate) fn matches(input: &str, name: &str) -> bool {
    let mut wanted = name.bytes().map(|b| b.to_ascii_lowercase());
    for b in input.bytes().filter(u8::is_ascii_alphanumeric) {
        if wanted.next() != Some(b.to_ascii_lowercase()) {
            return false;
        }
    }
    wanted.next().is_none()
}

/// The first `value` whose names include `input`, or
/// [`TimeError::UnknownName`](crate::TimeError::UnknownName).
pub(crate) fn lookup<T: Copy>(input: &str, table: &[(T, &[&str])]) -> Result<T, crate::TimeError> {
    table
        .iter()
        .find(|(_, names)| names.iter().any(|n| matches(input, n)))
        .map(|(value, _)| *value)
        .ok_or(crate::TimeError::UnknownName)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_ignores_case_and_punctuation_only() {
        assert!(matches("modified following", "ModifiedFollowing"));
        assert!(matches("MODIFIED-FOLLOWING", "ModifiedFollowing"));
        assert!(!matches("modified", "ModifiedFollowing"));
        assert!(!matches("modifiedfollowingx", "ModifiedFollowing"));
        assert!(!matches("", "Annual"));
    }
}
