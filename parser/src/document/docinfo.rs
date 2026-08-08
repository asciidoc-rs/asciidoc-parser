//! Resolves a document's [docinfo] content from the `docinfo` family of
//! attributes and a caller-supplied
//! [`DocinfoFileHandler`](crate::parser::DocinfoFileHandler).
//!
//! [docinfo]: https://docs.asciidoctor.org/asciidoc/latest/docinfo/

use crate::{
    Parser, SafeMode, Span,
    content::{Content, SubstitutionGroup, SubstitutionStep},
    document::InterpretedValue,
    parser::{CatalogResolver, ReferenceWarnings},
};

/// Where a [docinfo] file's content is injected into the converted output.
///
/// Each location corresponds to a distinct set of docinfo files (differentiated
/// by name) and a distinct insertion point in the output document.
///
/// [docinfo]: https://docs.asciidoctor.org/asciidoc/latest/docinfo/
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
        //
        // When `docinfo` itself is unset, the legacy `docinfo1`/`docinfo2`
        // standalone boolean attributes are consulted as a fallback: `docinfo1`
        // is equivalent to `docinfo=shared`, and `docinfo2` is equivalent to
        // `docinfo=private,shared`.
        // A `docinfo` value that resolves to exactly "private" -- whether
        // from a bare `:docinfo:` entry or an explicit `:docinfo: private`
        // (both read back as the literal `Value("private")` here, since
        // `Parser::attribute_value` already resolves a bare boolean to its
        // registered default) -- does not preclude the legacy
        // `docinfo1`/`docinfo2` booleans from also contributing their
        // shared-file component. Asciidoctor unions them rather than letting
        // the literal `docinfo` value suppress the derived "shared"
        // component: `docinfo docinfo2` resolves the same as `docinfo2`
        // alone, regardless of which was set first. A `docinfo` value with
        // any other content (e.g. `shared-head`) is left as the caller's
        // explicit, complete scope.
        let union_legacy_shared = |mut tokens: Vec<String>| -> Vec<String> {
            if tokens == ["private"]
                && (parser.is_attribute_set("docinfo1") || parser.is_attribute_set("docinfo2"))
            {
                tokens.push("shared".to_string());
            }
            tokens
        };

        let tokens: Vec<String> = match parser.attribute_value("docinfo") {
            InterpretedValue::Unset => {
                if parser.is_attribute_set("docinfo2") {
                    vec!["private".to_string(), "shared".to_string()]
                } else if parser.is_attribute_set("docinfo1") {
                    vec!["shared".to_string()]
                } else {
                    return Self::default();
                }
            }
            // `docinfo` has a registered default (`"private"`, set in
            // `built_in_default_values`), so `Parser::attribute_value`
            // always resolves a bare boolean to that default -- see the
            // `Value` arm below -- and never returns a bare `Set` for it.
            // This arm exists only to satisfy match exhaustiveness.
            InterpretedValue::Set => unreachable!(
                "docinfo has a registered default, so Parser::attribute_value never returns \
                 InterpretedValue::Set for it"
            ),
            InterpretedValue::Value(v) => union_legacy_shared(
                v.split(',')
                    .map(|t| t.trim().to_ascii_lowercase())
                    .filter(|t| !t.is_empty())
                    .collect(),
            ),
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

        let docinfo_subs = docinfosubs_steps(parser);

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

            if docinfo_subs.steps().is_empty() {
                return joined;
            }

            // Substitution may record `warn`-mode warnings whose offsets refer
            // to the docinfo text, not the document source. Discard them so
            // they are not reported against the document.
            let saved = parser.substitution_warnings_len();
            let mut content = Content::from(Span::new(&joined));
            docinfo_subs.apply(&mut content, parser, None);

            // A `docinfosubs` list that includes `macros` may have discovered
            // cross-references (`<<id>>`, `xref:id[…]`), left deferred with an
            // unresolved-fallback rendering. Docinfo content isn't part of the
            // document's own block tree, so the later document-wide
            // `resolve_against_own_catalog` pass never visits it; resolve it
            // here instead, directly against the parser's catalog. This is
            // safe because `Docinfo::resolve` runs only after the entire
            // document body has been parsed, so the catalog is already
            // complete. Any unresolved-reference warning is discarded for the
            // same reason the substitution warnings above are: its span
            // refers to the docinfo text, not the document source.
            if content.has_unresolved_refs() {
                let catalog = parser.catalog();
                let resolver = CatalogResolver::new(&catalog);
                let mut ref_warnings = ReferenceWarnings::default();
                content.resolve_references(&resolver, &*parser.renderer, &mut ref_warnings);
            }

            let substituted = content.rendered_owned();
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

/// Resolves the ordered list of substitution steps applied to docinfo
/// content, per the `docinfosubs` attribute.
///
/// When `docinfosubs` is unset it has an implied default of `attributes`, so
/// only the attribute-references substitution is applied. When set with no
/// value (a bare `:docinfosubs:`), no substitutions are applied. When set
/// with a value, the comma-separated list is parsed with the same vocabulary
/// and ordering rules as a block's `subs` attribute (see
/// [`SubstitutionGroup::from_custom_string`]); unrecognized names are
/// silently ignored.
fn docinfosubs_steps(parser: &Parser) -> SubstitutionGroup {
    match parser.attribute_value("docinfosubs") {
        InterpretedValue::Unset => {
            SubstitutionGroup::Custom(vec![SubstitutionStep::AttributeReferences])
        }
        InterpretedValue::Set => SubstitutionGroup::Custom(vec![]),
        InterpretedValue::Value(v) => SubstitutionGroup::from_custom_string(None, &v).0,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        Parser, SafeMode,
        document::DocinfoLocation,
        parser::{DocinfoFileHandler, ModificationContext},
    };

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
    fn drop_line_removes_docinfo_lines_with_an_unset_attribute() {
        // A reference to an attribute explicitly unset via a document
        // `:name!:` entry is dropped just like one that was never assigned at
        // all (issue #1117).
        let head = head_for(
            "= Doc\n:attribute-missing: drop-line\n:license-url!:\n:docinfo: shared-head\n\nBody.",
            &[("docinfo.html", "keep one\n{license-url}\nkeep two")],
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

    #[test]
    fn docinfo1_is_equivalent_to_shared_docinfo() {
        // `docinfo1` is a legacy standalone boolean attribute equivalent to
        // `docinfo=shared` (issue #1115): only the shared docinfo file is
        // included, not a document-private one.
        let head = head_for(
            "= Doc\n:docinfo1:\n\nBody.",
            &[
                ("docinfo.html", "SHARED"),
                ("mydoc-docinfo.html", "PRIVATE"),
            ],
        );
        assert_eq!(head, "SHARED");
    }

    #[test]
    fn docinfo2_is_equivalent_to_private_and_shared_docinfo() {
        // `docinfo2` is a legacy standalone boolean attribute equivalent to
        // `docinfo=private,shared` (issue #1115): both the shared and
        // document-private docinfo files are included, shared first.
        let head = head_for(
            "= Doc\n:docinfo2:\n\nBody.",
            &[
                ("docinfo.html", "SHARED"),
                ("mydoc-docinfo.html", "PRIVATE"),
            ],
        );
        assert_eq!(head, "SHARED\nPRIVATE");
    }

    #[test]
    fn bare_docinfo_and_docinfo2_union_to_private_and_shared() {
        // A bare `:docinfo:` ("private" only) and `:docinfo2:` ("private" +
        // "shared") set together union their effects rather than one
        // suppressing the other's shared-file component: matches
        // Asciidoctor's `docinfo docinfo2` test case, which is identical to
        // `docinfo2` alone.
        let head = head_for(
            "= Doc\n:docinfo:\n:docinfo2:\n\nBody.",
            &[
                ("docinfo.html", "SHARED"),
                ("mydoc-docinfo.html", "PRIVATE"),
            ],
        );
        assert_eq!(head, "SHARED\nPRIVATE");

        // Order does not matter.
        let head = head_for(
            "= Doc\n:docinfo2:\n:docinfo:\n\nBody.",
            &[
                ("docinfo.html", "SHARED"),
                ("mydoc-docinfo.html", "PRIVATE"),
            ],
        );
        assert_eq!(head, "SHARED\nPRIVATE");
    }

    #[test]
    fn api_set_bare_docinfo_unions_with_docinfo2() {
        // A bare `docinfo` boolean set via the intrinsic-attribute API
        // (rather than a document `:docinfo:` header entry) still reads back
        // as `Value("private")` -- `Parser::attribute_value` resolves a bare
        // `Set` to its registered default regardless of how it was set --
        // so it unions with a document `:docinfo2:` entry's shared scope the
        // same way.
        let head = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("mydoc.adoc")
            .with_docinfo_file_handler(MapHandler::new(&[
                ("docinfo.html", "SHARED"),
                ("mydoc-docinfo.html", "PRIVATE"),
            ]))
            .with_intrinsic_attribute_bool("docinfo", true, ModificationContext::Anywhere)
            .parse("= Doc\n:docinfo2:\n\nBody.")
            .docinfo(DocinfoLocation::Head)
            .to_string();
        assert_eq!(head, "SHARED\nPRIVATE");
    }

    #[test]
    fn bare_docinfo_and_docinfo1_union_to_private_and_shared() {
        // Same union behavior applies against the `docinfo1` legacy toggle
        // ("shared" only): the bare `docinfo` attribute's "private" scope is
        // combined with it rather than suppressing it.
        let head = head_for(
            "= Doc\n:docinfo:\n:docinfo1:\n\nBody.",
            &[
                ("docinfo.html", "SHARED"),
                ("mydoc-docinfo.html", "PRIVATE"),
            ],
        );
        assert_eq!(head, "SHARED\nPRIVATE");
    }

    #[test]
    fn explicit_docinfo_attribute_takes_precedence_over_legacy_toggles() {
        // When `docinfo` is explicitly set, the legacy `docinfo1`/`docinfo2`
        // toggles are not consulted at all.
        let head = head_for(
            "= Doc\n:docinfo: shared-head\n:docinfo2:\n\nBody.",
            &[
                ("docinfo.html", "SHARED"),
                ("mydoc-docinfo.html", "PRIVATE"),
            ],
        );
        assert_eq!(head, "SHARED");
    }

    #[test]
    fn docinfosubs_multiple_tokens_apply_additional_steps() {
        // `docinfosubs=attributes,replacements` (issue #1116) applies both the
        // attribute-references substitution and the character-replacements
        // substitution, not just attribute references: `(C)` becomes the
        // copyright character reference.
        let head = head_for(
            "= Doc\n:license-url: https://example.org\n:docinfo: shared-head\n:docinfosubs: attributes,replacements\n\nBody.",
            &[("docinfo.html", "{license-url} (C)")],
        );
        assert_eq!(head, "https://example.org &#169;");
    }

    #[test]
    fn docinfosubs_macros_resolves_cross_references_against_the_document_catalog() {
        // `docinfosubs=macros` runs the macros substitution on docinfo
        // content, which can discover a cross-reference. Docinfo content
        // isn't part of the document's own block tree, so the reference must
        // still resolve against the document's catalog rather than being left
        // as unresolved-fallback text.
        let head = head_for(
            "= Doc\n:docinfo: shared-head\n:docinfosubs: macros\n\n[#section-a]\n== Section A\n\ncontent",
            &[("docinfo.html", "xref:section-a[]")],
        );
        assert_eq!(head, r##"<a href="#section-a">Section A</a>"##);
    }

    #[test]
    fn docinfosubs_replacements_only_skips_attribute_substitution() {
        // `docinfosubs=replacements` names only the replacements step, so
        // attribute references are left untouched while `(C)` is still
        // replaced.
        let head = head_for(
            "= Doc\n:license-url: https://example.org\n:docinfo: shared-head\n:docinfosubs: replacements\n\nBody.",
            &[("docinfo.html", "{license-url} (C)")],
        );
        assert_eq!(head, "{license-url} &#169;");
    }
}
