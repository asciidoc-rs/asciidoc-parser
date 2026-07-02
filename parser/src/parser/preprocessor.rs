use std::{borrow::Cow, sync::LazyLock};

use regex::{Regex, Replacer};

use crate::{
    HasSpan, Parser, SafeMode, Span,
    attributes::{Attrlist, AttrlistContext},
    document::{Attribute, InterpretedValue},
    parser::{DeferredWarning, SourceLine, SourceMap},
    span::MatchedItem,
    warnings::{Warning, WarningType},
};

/// Given a root file (initial input to `Parser::parse`), convert this into a
/// `String` suitable for regular parsing and a `SourceMap` that maps line
/// numbers in the parse-ready text back to original input file and line
/// numbers.
///
/// This function handles [include file] and [conditional] processing.
///
/// Any warnings raised during preprocessing (e.g. an include directive whose
/// target could not be resolved) are returned as [`DeferredWarning`]s, located
/// by byte offset within the returned (preprocessed) source. `Document::parse`
/// reconstitutes them into spanned [`Warning`]s once it owns that source.
///
/// [include file]: https://docs.asciidoctor.org/asciidoc/latest/directives/include/
/// [conditional]: https://docs.asciidoctor.org/asciidoc/latest/directives/conditionals/
pub(crate) fn preprocess(
    source: &str,
    parser: &Parser,
) -> (String, SourceMap, Vec<DeferredWarning>) {
    // Short-circuit if the original source document has no pre-processor
    // directives.
    if !source.starts_with("include::")
        && !source.starts_with("if")
        && !source.contains("\ninclude::")
        && !source.contains("\nif")
        && !source.starts_with("\\include::")
        && !source.contains("\n\\include::")
        && parser.primary_file_name.is_none()
    {
        return (source.to_owned(), SourceMap::default(), vec![]);
    }

    // We use a temporary clone of the parser to track document attribute values
    // while parsing. These get recalculated again later when doing the full
    // document parsing.
    let mut temp_parser = parser.clone();
    let mut state = PreprocessorState::new(&mut temp_parser);
    state.process_adoc_include(source, parser.primary_file_name.as_deref());

    (state.output, state.source_map, state.warnings)
}

#[derive(Debug)]
struct PreprocessorState<'p> {
    parser: &'p mut Parser,
    in_document_header: bool,
    can_have_attribute: bool,
    include_depth: usize,
    output_line_number: usize,
    output: String,
    source_map: SourceMap,
    warnings: Vec<DeferredWarning>,
}

impl<'p> PreprocessorState<'p> {
    fn new(parser: &'p mut Parser) -> Self {
        Self {
            parser,
            in_document_header: true,
            can_have_attribute: true,
            include_depth: 0,
            output_line_number: 1,
            output: String::new(),
            source_map: SourceMap::default(),
            warnings: vec![],
        }
    }

    fn process_adoc_include(&mut self, source: &str, file_name: Option<&str>) {
        self.include_depth += 1;

        let mut has_reported_file = file_name.is_none();
        let mut source_span = Span::new(source);

        while !source_span.is_empty() {
            let original_source = source_span;

            let MatchedItem { item: line, after } = source_span.take_line();
            source_span = after;

            let source_line_number = line.line();

            if self.can_have_attribute
                && line.starts_with(':')
                && (line.ends_with(':') || line.contains(": "))
                && let Some(attr) = Attribute::parse(original_source, self.parser)
            {
                // Process attribute entries so they're available for include directives. NOTE:
                // We ignore warnings here since this is a quick pass through the content.
                // Later, `Block::parse` will see the same warnings, if they occur, and will
                // actually record them.
                if !has_reported_file {
                    has_reported_file = true;
                    self.source_map.append(
                        self.output_line_number,
                        SourceLine(to_owned(file_name), source_line_number),
                    );
                }

                let mut warnings: Vec<Warning> = vec![];
                self.parser
                    .set_attribute_from_body(&attr.item, &mut warnings);

                self.output.push_str(attr.item.span().data());
                self.output.push('\n');

                self.output_line_number += attr
                    .item
                    .span()
                    .data()
                    .as_bytes()
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count()
                    + 1;

                source_span = attr.after;
            } else if line.starts_with("include::")
                && let Some(caps) = INCLUDE_DIRECTIVE.captures(line.data())
            {
                let target = self.substitute_attributes(&caps[1]);

                if self.parser.safe >= SafeMode::Secure {
                    // The include directive is disabled at `SafeMode::Secure`
                    // and above (the default): rather than embed the contents of
                    // an arbitrary file, the directive is converted to a link to
                    // its target, matching Asciidoctor. The include file handler
                    // is never consulted in this case.
                    if !has_reported_file {
                        has_reported_file = true;
                        self.source_map.append(
                            self.output_line_number,
                            SourceLine(to_owned(file_name), source_line_number),
                        );
                    }

                    let replacement = format!("link:{target}[role=include]");
                    self.output_line_number += 1;
                    self.output.push_str(&replacement);
                    self.output.push('\n');

                    continue;
                }

                let attrlist = caps
                    .get(2)
                    .map(|attrlist| {
                        let span = Span::new(attrlist.as_str());
                        Attrlist::parse(span, self.parser, AttrlistContext::Inline)
                            .item
                            .item
                    })
                    .unwrap_or_default();

                if let Some(include_text) =
                    self.parser.include_file_handler.as_ref().and_then(|ifh| {
                        ifh.resolve_target(file_name, &target, &attrlist, self.parser)
                    })
                {
                    if is_asciidoc_file(&target) {
                        // AsciiDoc files are run through the preprocessor, so the
                        // include (and other) directives they contain are
                        // interpreted.
                        self.process_adoc_include(&include_text, Some(&target));
                    } else {
                        // Non-AsciiDoc files are merged verbatim; the preprocessor
                        // does not interpret any AsciiDoc directives within them
                        // (matching Asciidoctor).
                        self.process_nonadoc_include(&include_text, Some(&target));
                    }

                    // Re-report the including file if there's more content.
                    has_reported_file = false;
                } else if attrlist.has_option("optional") {
                    // `opts=optional`: a target that can't be resolved is dropped
                    // silently — neither the "Unresolved directive" text nor a
                    // warning is produced (matching Asciidoctor). Nothing is
                    // emitted for this line; re-anchor the source map so the lines
                    // that follow map back to their correct original line numbers.
                    has_reported_file = false;
                } else {
                    // The target could not be resolved. Replace the directive with
                    // an "Unresolved directive" message in the output (as
                    // Asciidoctor does) and record a warning pointing at that
                    // message. The warning is located by byte offset because the
                    // output it refers to is not yet owned; `Document::parse`
                    // reconstitutes it into a spanned `Warning`.
                    let replacement = format!(
                        "Unresolved directive in {file_name} - {line}",
                        file_name = file_name.unwrap_or("(root file)",),
                        line = line.data(),
                    );

                    self.warnings.push(DeferredWarning {
                        offset: self.output.len(),
                        len: replacement.len(),
                        warning: WarningType::IncludeFileNotFound(target),
                    });

                    self.output_line_number += 1;
                    self.output.push_str(&replacement);
                    self.output.push('\n');
                }
            } else {
                // If none of the above apply, add the line to output.
                if !has_reported_file {
                    has_reported_file = true;
                    self.source_map.append(
                        self.output_line_number,
                        SourceLine(to_owned(file_name), source_line_number),
                    );
                }

                if line.is_empty() {
                    self.in_document_header = false;
                    self.can_have_attribute = true;
                } else if !self.in_document_header {
                    self.can_have_attribute = false;
                }

                // An escaped include directive (e.g. `\include::foo[]`) is not
                // processed. The leading backslash is removed and the remainder is
                // emitted literally, matching Asciidoctor. The backslash is only
                // removed when what follows is actually an include directive; a
                // backslash followed by anything else is left untouched.
                let line_text = if line.starts_with("\\include::")
                    && INCLUDE_DIRECTIVE.is_match(&line.data()[1..])
                {
                    &line.data()[1..]
                } else {
                    line.data()
                };

                self.output_line_number += 1;
                self.output.push_str(line_text);
                self.output.push('\n');
            }
        }

        self.include_depth -= 1;
    }

    /// Merge the content of a non-AsciiDoc include verbatim.
    ///
    /// Unlike [`process_adoc_include`], the content is not scanned for
    /// preprocessor directives (nested includes, attribute entries, etc.); it
    /// is inserted as-is, subject only to the line-ending normalization
    /// that [`Span::take_line`] already performs. This mirrors how
    /// Asciidoctor treats files that are not recognized as AsciiDoc.
    ///
    /// [`process_adoc_include`]: Self::process_adoc_include
    fn process_nonadoc_include(&mut self, source: &str, file_name: Option<&str>) {
        let mut source_span = Span::new(source);
        let mut has_reported_file = false;

        while !source_span.is_empty() {
            let MatchedItem { item: line, after } = source_span.take_line();
            source_span = after;

            if !has_reported_file {
                has_reported_file = true;
                self.source_map.append(
                    self.output_line_number,
                    SourceLine(to_owned(file_name), line.line()),
                );
            }

            if line.is_empty() {
                self.in_document_header = false;
                self.can_have_attribute = true;
            } else if !self.in_document_header {
                self.can_have_attribute = false;
            }

            self.output_line_number += 1;
            self.output.push_str(line.data());
            self.output.push('\n');
        }
    }

    /// Apply attribute substitution to a string, replacing {attribute-name}
    /// patterns with their corresponding values from the parser.
    fn substitute_attributes(&self, input: &str) -> String {
        if !input.contains('{') {
            return input.to_string();
        }

        #[derive(Debug)]
        struct AttributeReplacer<'p>(&'p Parser);

        impl Replacer for AttributeReplacer<'_> {
            fn replace_append(&mut self, caps: &regex::Captures<'_>, dest: &mut String) {
                let attr_name = &caps[1];

                if !self.0.has_attribute(attr_name) {
                    dest.push_str(&caps[0]);
                    return;
                }

                if caps[0].starts_with('\\') {
                    dest.push_str(&caps[0][1..]);
                    return;
                }

                if let InterpretedValue::Value(value) = self.0.attribute_value(attr_name) {
                    dest.push_str(value.as_ref());
                }
            }
        }

        let result: Cow<'_, str> = input.into();

        if let Cow::Owned(new_result) =
            ATTRIBUTE_REFERENCE.replace_all(&result, AttributeReplacer(self.parser))
        {
            new_result
        } else {
            input.to_string()
        }
    }
}

fn to_owned(maybe_file_name: Option<&str>) -> Option<String> {
    maybe_file_name.map(|n| n.to_string())
}

/// Returns `true` if `target` names an AsciiDoc file, based on its extension.
///
/// Per the [include directive] spec, a file is treated as AsciiDoc if it has
/// one of these extensions: `.asciidoc`, `.adoc`, `.ad`, `.asc`, or `.txt`. The
/// comparison is case-sensitive, and a target with no extension is not
/// considered AsciiDoc — both matching Asciidoctor.
///
/// [include directive]: https://docs.asciidoctor.org/asciidoc/latest/directives/include/#include-nonasciidoc
fn is_asciidoc_file(target: &str) -> bool {
    const ASCIIDOC_EXTENSIONS: [&str; 5] = ["asciidoc", "adoc", "ad", "asc", "txt"];

    let file_name = target.rsplit('/').next().unwrap_or(target);

    match file_name.rsplit_once('.') {
        // A leading dot (e.g. `.adoc`) denotes a hidden file with no extension.
        Some((stem, ext)) if !stem.is_empty() => ASCIIDOC_EXTENSIONS.contains(&ext),
        _ => false,
    }
}

static INCLUDE_DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)                      # Extended (verbose) mode

        ^                           # Start of string

        include::                   # Literal 'include::' macro prefix

        (                           # (1) Target path
            [^\s\[]                   #   First char: not space or '['
            (?: [^\[]* [^\s\[] )?     #   Optional middle part ending with non-space/non-'['
        )                           # end capture group 1

        \[                          # Literal '[' starting the attributes block

        ([^\]].+)?                  # (2) Optional contents inside brackets (lazy by default)

        \]                          # Literal closing bracket

        $                           # End of line
        "#,
    )
    .unwrap()
});

static ATTRIBUTE_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"\\?\{([A-Za-z0-9_][A-Za-z0-9_-]*)\}"#).unwrap()
});

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::unwrap_used)]

    use crate::{
        SafeMode,
        parser::{SourceLine, preprocessor::preprocess},
        tests::{fixtures::inline_file_handler::InlineFileHandler, prelude::*},
    };

    #[test]
    fn no_preprocessor_directives() {
        let source =
            "= Document Title\n\nThis is a simple document with no includes or conditionals.";
        let parser = Parser::default().with_primary_file_name("test.adoc");

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "= Document Title\n\nThis is a simple document with no includes or conditionals.\n"
        );

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("test.adoc".to_owned()), 1))
        );
    }

    #[test]
    fn simple_include_directive() {
        let source = "= Document Title\n\ninclude::shared.adoc[]\n\nMore content.";

        let handler = InlineFileHandler::from_pairs([(
            "shared.adoc",
            "This is shared content.\n\nWith multiple lines.\n",
        )]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "= Document Title\n\nThis is shared content.\n\nWith multiple lines.\n\nMore content.\n"
        );

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("main.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(2),
            Some(SourceLine(Some("main.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(3),
            Some(SourceLine(Some("shared.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("shared.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(5),
            Some(SourceLine(Some("shared.adoc".to_owned()), 3))
        );
        assert_eq!(
            source_map.original_file_and_line(6),
            Some(SourceLine(Some("main.adoc".to_owned()), 4))
        );
        assert_eq!(
            source_map.original_file_and_line(7),
            Some(SourceLine(Some("main.adoc".to_owned()), 5))
        );
    }

    #[test]
    fn include_directive_at_start() {
        let source = "include::header.adoc[]\n\n= Document Title\n\nContent here.";

        let handler =
            InlineFileHandler::from_pairs([("header.adoc", ":author: John Doe\n:version: 1.0")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            ":author: John Doe\n:version: 1.0\n\n= Document Title\n\nContent here.\n"
        );

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("header.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(2),
            Some(SourceLine(Some("header.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(3),
            Some(SourceLine(Some("main.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("main.adoc".to_owned()), 3))
        );
        assert_eq!(
            source_map.original_file_and_line(5),
            Some(SourceLine(Some("main.adoc".to_owned()), 4))
        );
        assert_eq!(
            source_map.original_file_and_line(6),
            Some(SourceLine(Some("main.adoc".to_owned()), 5))
        );
    }

    #[test]
    fn nested_includes() {
        let source =
            "= Document Title\n\ninclude::chapter1.adoc[]\n\n(a little more of root document)";

        let handler = InlineFileHandler::from_pairs([
            (
                "chapter1.adoc",
                "== Chapter 1\n\ninclude::section1.adoc[]\n\n(a little more of chapter 1)",
            ),
            ("section1.adoc", "=== Section 1\n\nContent here."),
        ]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "= Document Title\n\n== Chapter 1\n\n=== Section 1\n\nContent here.\n\n(a little more of chapter 1)\n\n(a little more of root document)\n"
        );

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("main.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(2),
            Some(SourceLine(Some("main.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(3),
            Some(SourceLine(Some("chapter1.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("chapter1.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(5),
            Some(SourceLine(Some("section1.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(6),
            Some(SourceLine(Some("section1.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(7),
            Some(SourceLine(Some("section1.adoc".to_owned()), 3))
        );
        assert_eq!(
            source_map.original_file_and_line(8),
            Some(SourceLine(Some("chapter1.adoc".to_owned()), 4))
        );
        assert_eq!(
            source_map.original_file_and_line(9),
            Some(SourceLine(Some("chapter1.adoc".to_owned()), 5))
        );
        assert_eq!(
            source_map.original_file_and_line(10),
            Some(SourceLine(Some("main.adoc".to_owned()), 4))
        );
    }

    #[test]
    fn include_with_missing_file() {
        let source = "= Document Title\n\ninclude::missing.adoc[]\n\nMore content.";

        // Handler doesn't provide missing.adoc.
        let handler = InlineFileHandler::from_pairs([("other.adoc", "Other content")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "= Document Title\n\nUnresolved directive in main.adoc - include::missing.adoc[]\n\nMore content.\n"
        );

        // A warning is recorded for the unresolved include, pointing at the
        // "Unresolved directive" text in the output.
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::IncludeFileNotFound("missing.adoc".to_owned())
        );
        assert_eq!(
            &processed_source[warnings[0].offset..warnings[0].offset + warnings[0].len],
            "Unresolved directive in main.adoc - include::missing.adoc[]"
        );

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("main.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(2),
            Some(SourceLine(Some("main.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(3),
            Some(SourceLine(Some("main.adoc".to_owned()), 3))
        );
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("main.adoc".to_owned()), 4))
        );
    }

    #[test]
    fn empty_file_with_include() {
        let source = "include::entire-doc.adoc[]";

        let handler = InlineFileHandler::from_pairs([(
            "entire-doc.adoc",
            "= Full Document\n\n== Chapter 1\n\nContent here.",
        )]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "= Full Document\n\n== Chapter 1\n\nContent here.\n"
        );

        // Since the main file only contains an include directive,
        // all content comes from the included file.
        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("entire-doc.adoc".to_owned()), 1))
        );
    }

    #[test]
    fn no_include_handler() {
        let source = "= Document Title\n\ninclude::missing.adoc[]\n\nMore content.";

        // NOTE: No include file handler provided.
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc");

        let (processed_source, source_map, warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "= Document Title\n\nUnresolved directive in main.adoc - include::missing.adoc[]\n\nMore content.\n"
        );

        // With no handler at all, the include is likewise unresolved and warned.
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::IncludeFileNotFound("missing.adoc".to_owned())
        );

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("main.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(2),
            Some(SourceLine(Some("main.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(3),
            Some(SourceLine(Some("main.adoc".to_owned()), 3))
        );
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("main.adoc".to_owned()), 4))
        );
    }

    #[test]
    fn asciidoc_file_recognition() {
        use super::is_asciidoc_file;

        // Recognized AsciiDoc extensions.
        assert!(is_asciidoc_file("foo.asciidoc"));
        assert!(is_asciidoc_file("foo.adoc"));
        assert!(is_asciidoc_file("foo.ad"));
        assert!(is_asciidoc_file("foo.asc"));
        assert!(is_asciidoc_file("foo.txt"));
        assert!(is_asciidoc_file("path/to/foo.adoc"));
        assert!(is_asciidoc_file("a.b.adoc"));

        // Not AsciiDoc.
        assert!(!is_asciidoc_file("foo.csv"));
        assert!(!is_asciidoc_file("foo.rb"));
        assert!(!is_asciidoc_file("path/to/data.csv"));
        assert!(!is_asciidoc_file("foo")); // no extension
        assert!(!is_asciidoc_file("foo.ADOC")); // case-sensitive
        assert!(!is_asciidoc_file(".adoc")); // hidden file, no stem
    }

    #[test]
    fn asciidoc_include_processes_nested_directives() {
        // An included AsciiDoc file is run through the preprocessor, so a nested
        // include within it is expanded.
        let source = "include::outer.adoc[]";

        let handler = InlineFileHandler::from_pairs([
            ("outer.adoc", "Top.\ninclude::inner.adoc[]"),
            ("inner.adoc", "Nested."),
        ]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, _source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(processed_source, "Top.\nNested.\n");
    }

    #[test]
    fn non_asciidoc_include_merged_verbatim() {
        // A non-AsciiDoc file (here `.csv`) is merged verbatim: a nested include
        // directive within it is left as literal text, not expanded.
        let source = "include::data.csv[]";

        let handler = InlineFileHandler::from_pairs([
            ("data.csv", "a,b\ninclude::inner.adoc[]"),
            ("inner.adoc", "SHOULD NOT APPEAR"),
        ]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, warnings) = preprocess(source, &parser);

        assert_eq!(processed_source, "a,b\ninclude::inner.adoc[]\n");
        assert!(warnings.is_empty());

        // The verbatim content maps back to the non-AsciiDoc file's lines.
        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("data.csv".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(2),
            Some(SourceLine(Some("data.csv".to_owned()), 2))
        );
    }

    #[test]
    fn non_asciidoc_include_in_body_tracks_header_state() {
        // A non-AsciiDoc include placed in the document body (so the preprocessor
        // is past the header) whose content includes a blank line exercises the
        // verbatim path's header-state updates for both blank and non-blank lines.
        let source = "Body.\n\ninclude::data.csv[]";

        let handler = InlineFileHandler::from_pairs([("data.csv", "row one\n\nrow two")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, _source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(processed_source, "Body.\n\nrow one\n\nrow two\n");
    }

    #[test]
    fn optional_include_dropped_silently() {
        // `opts=optional` drops an unresolved include with no output text and no
        // warning, while keeping the source map aligned for the lines that follow.
        let source = "Before.\n\ninclude::missing.adoc[opts=optional]\n\nAfter.";

        // Handler doesn't provide missing.adoc.
        let handler = InlineFileHandler::from_pairs([("other.adoc", "Other content")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, warnings) = preprocess(source, &parser);

        // The directive line is gone; no "Unresolved directive" text is inserted.
        assert_eq!(processed_source, "Before.\n\n\nAfter.\n");
        assert!(warnings.is_empty());

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("main.adoc".to_owned()), 1)) // Before.
        );
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("main.adoc".to_owned()), 5)) // After.
        );
    }

    #[test]
    fn escaped_include_directive() {
        // An escaped include directive is not processed. The leading backslash is
        // stripped and the remainder is emitted literally (matching Asciidoctor).
        let source = "Before.\n\n\\include::partial.adoc[]\n\nAfter.";

        let handler = InlineFileHandler::from_pairs([("partial.adoc", "SHOULD NOT APPEAR")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "Before.\n\ninclude::partial.adoc[]\n\nAfter.\n"
        );

        assert_eq!(
            source_map.original_file_and_line(3),
            Some(SourceLine(Some("main.adoc".to_owned()), 3))
        );
    }

    #[test]
    fn escaped_include_directive_without_primary_file() {
        // The backslash is stripped even when there is no primary file name (and
        // thus no include handler) so the escape behaves identically.
        let source = "\\include::partial.adoc[]";
        let parser = Parser::default();
        let (processed_source, _source_map, _warnings) = preprocess(source, &parser);
        assert_eq!(processed_source, "include::partial.adoc[]\n");
    }

    #[test]
    fn escaped_non_directive_is_unchanged() {
        // A backslash followed by something that is not a valid include directive
        // (here, no attribute brackets) is left untouched.
        let source = "\\include::partial.adoc";
        let parser = Parser::default().with_primary_file_name("main.adoc");
        let (processed_source, _source_map, _warnings) = preprocess(source, &parser);
        assert_eq!(processed_source, "\\include::partial.adoc\n");
    }

    #[test]
    fn double_backslash_include_is_unchanged() {
        // Only a single leading backslash is treated as an escape; a double
        // backslash is left as-is.
        let source = "\\\\include::partial.adoc[]";
        let parser = Parser::default().with_primary_file_name("main.adoc");
        let (processed_source, _source_map, _warnings) = preprocess(source, &parser);
        assert_eq!(processed_source, "\\\\include::partial.adoc[]\n");
    }

    #[test]
    fn multiple_includes_same_line() {
        let source = "include::part1.adoc[] include::part2.adoc[]";

        let handler = InlineFileHandler::from_pairs([
            ("part1.adoc", "First part"),
            ("part2.adoc", "Second part"),
        ]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "include::part1.adoc[] include::part2.adoc[]\n"
        );
        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("main.adoc".to_owned()), 1))
        );
    }

    #[test]
    fn attribute_substitution_in_include_target() {
        let source =
            ":fixturesdir: fixtures\n:ext: adoc\n\ninclude::{fixturesdir}/include-file.{ext}[]";

        let handler = InlineFileHandler::from_pairs([(
            "fixtures/include-file.adoc",
            "This is included content.",
        )]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            ":fixturesdir: fixtures\n:ext: adoc\n\nThis is included content.\n"
        );

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("main.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(2),
            Some(SourceLine(Some("main.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(3),
            Some(SourceLine(Some("main.adoc".to_owned()), 3))
        );
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("fixtures/include-file.adoc".to_owned()), 1))
        );
    }

    #[test]
    fn multiple_attribute_substitution_in_include_target() {
        let source = ":dir: chapters\n:filename: intro\n:extension: adoc\n\ninclude::{dir}/{filename}.{extension}[]";

        let handler = InlineFileHandler::from_pairs([(
            "chapters/intro.adoc",
            "= Introduction\n\nWelcome to the guide.",
        )]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            ":dir: chapters\n:filename: intro\n:extension: adoc\n\n= Introduction\n\nWelcome to the guide.\n"
        );

        assert_eq!(
            source_map.original_file_and_line(5),
            Some(SourceLine(Some("chapters/intro.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(6),
            Some(SourceLine(Some("chapters/intro.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(7),
            Some(SourceLine(Some("chapters/intro.adoc".to_owned()), 3))
        );
    }

    #[test]
    fn missing_attribute_in_include_target() {
        let source = ":fixturesdir: fixtures\n\ninclude::{fixturesdir}/include-file.{missingext}[]";

        let handler = InlineFileHandler::from_pairs([
            (
                "fixtures/include-file.adoc",
                "This content won't be included.",
            ),
            ("fixtures/include-file.", "This shouldn't match either."),
        ]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            ":fixturesdir: fixtures\n\nUnresolved directive in main.adoc - include::{fixturesdir}/include-file.{missingext}[]\n"
        );

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("main.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(2),
            Some(SourceLine(Some("main.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(3),
            Some(SourceLine(Some("main.adoc".to_owned()), 3))
        );
    }

    #[test]
    fn attribute_substitution_with_nested_includes() {
        let source = ":basedir: content\n:format: adoc\n\ninclude::{basedir}/main.{format}[]";

        let handler = InlineFileHandler::from_pairs([
            (
                "content/main.adoc",
                ":partdir: parts\n\n== Main Chapter\n\ninclude::{partdir}/section1.{format}[]",
            ),
            (
                "parts/section1.adoc",
                "=== Section 1\n\nSection content here.",
            ),
        ]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            ":basedir: content\n:format: adoc\n\n:partdir: parts\n\n== Main Chapter\n\n=== Section 1\n\nSection content here.\n"
        );

        assert_eq!(
            source_map.original_file_and_line(8),
            Some(SourceLine(Some("parts/section1.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(9),
            Some(SourceLine(Some("parts/section1.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(10),
            Some(SourceLine(Some("parts/section1.adoc".to_owned()), 3))
        );
    }

    #[test]
    #[ignore]
    fn attribute_substitution_in_target_with_attrlist() {
        // TODO: Implement tag handling.
        let source = ":srcdir: examples\n:lang: java\n\ninclude::{srcdir}/hello.{lang}[tag=main]";

        let handler = InlineFileHandler::from_pairs([(
            "examples/hello.java",
            "// tag::main\npublic class Hello {}\n// end::main",
        )]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            ":srcdir: examples\n:lang: java\n\n// tag::main\npublic class Hello {}\n// end::main\n"
        );

        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("examples/hello.java".to_owned()), 1))
        );
    }

    #[test]
    fn attribute_substitution_with_multiline_attribute() {
        let source = ":longpath: very/long/path/to/some/ \\\nsubdirectory\n:ext: adoc\n\ninclude::{longpath}/file.{ext}[]";

        // TODO: This should be "very/long/path/to/some/subdirectory/file.adoc" (without
        // space) but the current Attribute::parse() incorrectly preserves the space
        // before the backslash in multi-line attribute continuation. This is a bug
        // that should be fixed.
        let handler = InlineFileHandler::from_pairs([(
            "very/long/path/to/some/ subdirectory/file.adoc",
            "Multi-line attribute worked!",
        )]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            ":longpath: very/long/path/to/some/ \\\nsubdirectory\n:ext: adoc\n\nMulti-line attribute worked!\n"
        );

        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(Some("main.adoc".to_owned()), 1))
        );
        assert_eq!(
            source_map.original_file_and_line(2),
            Some(SourceLine(Some("main.adoc".to_owned()), 2))
        );
        assert_eq!(
            source_map.original_file_and_line(3),
            Some(SourceLine(Some("main.adoc".to_owned()), 3))
        );
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("main.adoc".to_owned()), 4))
        );
        assert_eq!(
            source_map.original_file_and_line(5),
            Some(SourceLine(
                Some("very/long/path/to/some/ subdirectory/file.adoc".to_owned()),
                1
            ))
        );
    }
}
