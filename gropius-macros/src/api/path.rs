use std::collections::BTreeSet;

use proc_macro2_diagnostics::{Diagnostic, SpanDiagnosticExt};

use super::RawEndpoint;

/// Attempt to catch path-related runtime errors at compile-time.
///
/// For example, if a user defines two identical endpoints, that'll fail when
/// they try to create the router; we can replicate those conditions now and
/// emit a diagnostic instead.
///
/// We can also prevent some paths that `matchit` would be okay with, but that
/// are OpenAPI-incompatible.
pub(crate) fn validate(endpoints: &[RawEndpoint], errors: &mut Vec<Diagnostic>) {
    let mut router = matchit::Router::<()>::new();
    let mut seen = BTreeSet::new();
    let mut routes = BTreeSet::new();

    for ep in endpoints {
        let path = ep.path.value();
        let span = ep.path.span();

        if path.contains("{*") {
            errors.push(span.error("catch-all routes (`{*...}`) cannot be represented in OpenAPI"));
            continue;
        }

        if path.contains("{{") || path.contains("}}") {
            errors.push(span.error("escaped braces (`{{`, `}}`) are not supported in paths"));
            continue;
        }

        if !routes.insert((&ep.method, path.to_string())) {
            errors.push(span.error(format!("duplicate route `{} {path}`", ep.method)));
            continue;
        }

        // Endpoints can share a path under different methods, so validate each
        // distinct template once. matchit reports malformed paths and conflicts
        // between overlapping templates.
        if seen.insert(path.to_string()) {
            match router.insert(&path, ()) {
                Ok(()) => {}
                Err(matchit::InsertError::Conflict { with }) => {
                    errors.push(span.error(format!("path conflicts with `{with}`")));
                }
                Err(e) => errors.push(span.error(format!("invalid path: {e}"))),
            }
        }
    }
}

/// Returns the parameter names in a path template, in order of appearance.
///
/// Path templating follows the OpenAPI 3.2 ABNF:
///
/// ```abnf
/// path-segment                   = 1*( path-literal / template-expression )
/// template-expression            = "{" template-expression-param-name "}"
/// template-expression-param-name = 1*( %x00-7A / %x7C / %x7E-10FFFF )
/// ```
pub(crate) fn parameter_names(path: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = path;

    while let Some(open) = rest.find('{') {
        // Move to the start of the next `{...}` expression.
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
