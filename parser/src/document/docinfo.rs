//! Resolves a document's [docinfo] content from the `docinfo` family of
//! attributes and a caller-supplied
//! [`DocinfoFileHandler`](crate::parser::DocinfoFileHandler).
//!
//! [docinfo]: https://docs.asciidoctor.org/asciidoc/latest/docinfo/

use crate::{Parser, SafeMode, content::substitute_attributes_in_text, document::InterpretedValue};

/// Where a [docinfo] file's content is injected into the converted output.
///
/// Each location corresponds to a distinct set of docinfo files (differentiated
/// by name) and a distinct insertion point in the output document.
///
/// [docinfo]: https://docs.asciidoctor.org/asciidoc/latest/docinfo/
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum DocinfoLocation {
    /// Head docinfo: injected into the top of the document (appended to the
    /// HTML `<head>` element, or the DocBook root `<info>` element).
    Head,

    /// Header docinfo: injected at the start of the document body (immediately
    /// before the HTML header `<div>`).
    Header,

    /// Footer docinfo: injected at the end of the document body (immediately
    /// after the HTML footer `<div>`).
    Footer,
}

impl DocinfoLocation {
    /// The token used to enable this location in the `docinfo` attribute (e.g.
    /// the `header` in `private-header`).
    fn token(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Header => "header",
            Self::Footer => "footer",
        }
    }

    /// The infix added to the docinfo file name for this location (`-header` or
    /// `-footer`; head files have no infix).
    fn name_infix(self) -> &'static str {
        match self {
            Self::Head => "",
            Self::Header => "-header",
            Self::Footer => "-footer",
        }
    }
}

/// A document's resolved docinfo content, captured once the document's header
/// (and body) have been processed and the parser holds the document's final
/// attribute state.
///
/// The content for each location is already concatenated (shared file first,
/// then private, matching Asciidoctor) and has had `docinfosubs` substitutions
/// applied. An empty string means no docinfo applies to that location (e.g. no
/// handler was configured, the `docinfo` attribute did not enable it, or no
/// matching file was found).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub(crate) struct Docinfo {
    head: String,
    header: String,
    footer: String,
}

impl Docinfo {
    /// Returns the resolved docinfo content for `location` (an empty string
    /// when none applies).
    pub(crate) fn content(&self, location: DocinfoLocation) -> &str {
        match location {
            DocinfoLocation::Head => &self.head,
            DocinfoLocation::Header => &self.header,
            DocinfoLocation::Footer => &self.footer,
        }
    }

    /// Resolves a document's docinfo from a parser's current attribute state
    /// and its configured
    /// [`DocinfoFileHandler`](crate::parser::DocinfoFileHandler).
    ///
    /// Returns empty content when no handler is configured or the `docinfo`
    /// attribute is unset.
    pub(crate) fn resolve(parser: &Parser) -> Self {
        // Docinfo injects the contents of external files directly into the
        // output, so Asciidoctor disables it entirely at `SafeMode::Secure` and
        // above (the default). A caller who wants docinfo must relax the safe
        // mode via [`Parser::with_safe_mode`].
        if parser.safe_mode() >= SafeMode::Secure {
            return Self::default();
        }

        let Some(handler) = parser.docinfo_file_handler.as_ref() else {
            return Self::default();
        };

        // The `docinfo` attribute selects which scopes/locations apply. An unset
        // attribute means no docinfo; an empty value (a bare `:docinfo:`) is
        // equivalent to `private`.
        let tokens: Vec<String> = match parser.attribute_value("docinfo") {
            InterpretedValue::Unset => return Self::default(),
            InterpretedValue::Set => vec!["private".to_string()],
            InterpretedValue::Value(v) => v
                .split(',')
                .map(|t| t.trim().to_ascii_lowercase())
                .filter(|t| !t.is_empty())
                .collect(),
        };

        if tokens.is_empty() {
            return Self::default();
        }

        // Docinfo file names share the output file extension (`outfilesuffix`,
        // which always begins with a period and defaults to `.html`).
        let suffix = match parser.attribute_value("outfilesuffix") {
            InterpretedValue::Value(v) => v,
            _ => ".html".to_string(),
        };

        // When `docinfodir` is set, files are searched only there; otherwise the
        // document directory is searched (the handler owns path resolution).
        let docinfodir = match parser.attribute_value("docinfodir") {
            InterpretedValue::Value(v) => Some(v),
            _ => None,
        };

        // Private docinfo file names are derived from the document name, so
        // private scope is only available when a primary file name is known.
        let docname = parser.docname();

        let apply_attribute_subs = docinfosubs_includes_attributes(parser);

        let resolve_location = |location: DocinfoLocation| -> String {
            let token = location.token();
            let infix = location.name_infix();

            let shared_token = format!("shared-{token}");
            let private_token = format!("private-{token}");

            let shared_enabled = tokens.iter().any(|t| t == "shared" || *t == shared_token);
            let private_enabled = tokens.iter().any(|t| t == "private" || *t == private_token);

            // Shared content is concatenated before private content, matching
            // Asciidoctor's output order.
            let mut parts: Vec<String> = vec![];

            if shared_enabled {
                let file_name = format!("docinfo{infix}{suffix}");
                if let Some(content) =
                    handler.resolve_docinfo(docinfodir.as_deref(), &file_name, parser)
                {
                    parts.push(content);
                }
            }

            if private_enabled && let Some(docname) = docname.as_deref() {
                let file_name = format!("{docname}-docinfo{infix}{suffix}");
                if let Some(content) =
                    handler.resolve_docinfo(docinfodir.as_deref(), &file_name, parser)
                {
                    parts.push(content);
                }
            }

            if parts.is_empty() {
                return String::new();
            }

            let joined = parts.join("\n");

            if !apply_attribute_subs {
                return joined;
            }

            // Attribute substitution may record `warn`-mode warnings whose
            // offsets refer to the docinfo text, not the document source.
            // Discard them so they are not reported against the document.
            let saved = parser.substitution_warnings_len();
            let substituted = substitute_attributes_in_text(&joined, parser);
            parser.truncate_substitution_warnings(saved);
            substituted
        };

        Self {
            head: resolve_location(DocinfoLocation::Head),
            header: resolve_location(DocinfoLocation::Header),
            footer: resolve_location(DocinfoLocation::Footer),
        }
    }
}

/// Returns whether the `attributes` substitution should be applied to docinfo
/// content, per the `docinfosubs` attribute.
///
/// When `docinfosubs` is unset it has an implied default of `attributes`, so
/// substitution is applied. When set, substitution is applied only if the
/// comma-separated list names `attributes`.
fn docinfosubs_includes_attributes(parser: &Parser) -> bool {
    match parser.attribute_value("docinfosubs") {
        InterpretedValue::Unset => true,
        InterpretedValue::Set => false,
        InterpretedValue::Value(v) => v.split(',').any(|s| s.trim() == "attributes"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{Parser, SafeMode, document::DocinfoLocation, parser::DocinfoFileHandler};

    /// A minimal handler that resolves docinfo from a fixed file-name map.
    #[derive(Debug)]
    struct MapHandler(HashMap<String, String>);

    impl MapHandler {
        fn new(pairs: &[(&str, &str)]) -> Self {
            Self(
                pairs
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            )
        }
    }

    impl DocinfoFileHandler for MapHandler {
        fn resolve_docinfo(
            &self,
            _docinfodir: Option<&str>,
            file_name: &str,
            _parser: &Parser,
        ) -> Option<String> {
            self.0.get(file_name).cloned()
        }
    }

    fn head_for(src: &str, files: &[(&str, &str)]) -> String {
        // Docinfo is disabled at `SafeMode::Secure` (the default), so these
        // tests run in `Server` mode, where docinfo is resolved.
        Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("mydoc.adoc")
            .with_docinfo_file_handler(MapHandler::new(files))
            .parse(src)
            .docinfo(DocinfoLocation::Head)
            .to_string()
    }

    #[test]
    fn empty_docinfosubs_disables_substitution() {
        // A bare `:docinfosubs:` (set, but with no value) names no
        // substitutions, so attribute references are left untouched.
        let head = head_for(
            "= Doc\n:license-url: https://example.org\n:docinfo: shared-head\n:docinfosubs:\n\nBody.",
            &[("docinfo.html", "{license-url}")],
        );
        assert_eq!(head, "{license-url}");
    }

    #[test]
    fn blank_docinfo_value_resolves_to_no_locations() {
        // A `docinfo` value made up only of separators yields no tokens, so no
        // docinfo is applied.
        let head = head_for(
            "= Doc\n:docinfo: ,\n\nBody.",
            &[("docinfo.html", "X"), ("mydoc-docinfo.html", "Y")],
        );
        assert_eq!(head, "");
    }

    #[test]
    fn multi_line_content_mixes_references_and_plain_lines() {
        // A docinfo file whose lines mix attribute references with plain lines
        // exercises line-by-line substitution and re-joining of the output.
        let head = head_for(
            "= Doc\n:name: World\n:docinfo: shared-head\n\nBody.",
            &[(
                "docinfo.html",
                "<p>Hello {name}</p>\n<p>plain line</p>\n<p>{name} again</p>",
            )],
        );
        assert_eq!(
            head,
            "<p>Hello World</p>\n<p>plain line</p>\n<p>World again</p>"
        );
    }

    #[test]
    fn drop_line_removes_docinfo_lines_with_missing_references() {
        // With `attribute-missing=drop-line`, a docinfo line referencing a
        // missing attribute is dropped while the surrounding lines are kept.
        let head = head_for(
            "= Doc\n:attribute-missing: drop-line\n:docinfo: shared-head\n\nBody.",
            &[("docinfo.html", "keep one\n{nope}\nkeep two")],
        );
        assert_eq!(head, "keep one\nkeep two");
    }

    #[test]
    fn outfilesuffix_falls_back_to_html() {
        // A bare `:outfilesuffix:` (set, no value) is not a usable suffix, so
        // docinfo file names fall back to the `.html` default.
        let head = head_for(
            "= Doc\n:docinfo: shared-head\n:outfilesuffix:\n\nBody.",
            &[("docinfo.html", "HEAD")],
        );
        assert_eq!(head, "HEAD");
    }
}
