//! Parsing parameter names out of a path template.

/// Returns the parameter names in a path template, in order of appearance.
///
/// Path templating follows the OpenAPI 3.2 ABNF:
///
/// ```abnf
/// path-segment                   = 1*( path-literal / template-expression )
/// template-expression            = "{" template-expression-param-name "}"
/// template-expression-param-name = 1*( %x00-7A / %x7C / %x7E-10FFFF )
/// ```
///
/// So a parameter is the run of characters between a `{` and the next `}`.
/// A name cannot itself contain a brace, so the expressions never nest, and a
/// parameter may appear anywhere within a segment (for example, the `id` in
/// `/images/img-{id}.png`).
pub(crate) fn parameter_names(path: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = path;

    loop {
        // Move to the start of the next `{...}` expression.
        let Some(open) = rest.find('{') else {
            break;
        };
        rest = &rest[open + 1..];

        // The name runs up to the closing brace.
        let Some(close) = rest.find('}') else {
            break;
        };
        names.push(&rest[..close]);
        rest = &rest[close + 1..];
    }

    names
}

#[cfg(test)]
mod tests {
    use super::parameter_names;

    #[test]
    fn no_parameters() {
        assert!(parameter_names("/v1/widgets").is_empty());
    }

    #[test]
    fn one_parameter_per_segment() {
        assert_eq!(parameter_names("/v1/chairs/{year}/{id}"), ["year", "id"]);
    }

    #[test]
    fn parameter_within_a_segment() {
        assert_eq!(parameter_names("/images/img-{id}.png"), ["id"]);
    }
}
