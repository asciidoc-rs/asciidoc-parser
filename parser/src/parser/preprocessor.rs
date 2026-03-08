use std::{borrow::Cow, sync::LazyLock};

use regex::{Regex, Replacer};

use crate::{
    HasSpan, Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    document::{Attribute, InterpretedValue},
    parser::{SourceLine, SourceMap},
    span::MatchedItem,
    warnings::Warning,
};

/// Given a root file (initial input to `Parser::parse`), convert this into a
/// `String` suitable for regular parsing and a `SourceMap` that maps line
/// numbers in the parse-ready text back to original input file and line
/// numbers.
///
/// This function handles [include file] and [conditional] processing.
///
/// [include file]: https://docs.asciidoctor.org/asciidoc/latest/directives/include/
/// [conditional]: https://docs.asciidoctor.org/asciidoc/latest/directives/conditionals/
pub(crate) fn preprocess(source: &str, parser: &Parser) -> (String, SourceMap) {
    // Short-circuit if the original source document has no pre-processor
    // directives.
    if !source.starts_with("include::")
        && !source.starts_with("if")
        && !source.contains("\ninclude::")
        && !source.contains("\nif")
        && parser.primary_file_name.is_none()
    {
        return (source.to_owned(), SourceMap::default());
    }

    // We use a temporary clone of the parser to track document attribute values
    // while parsing. These get recalculated again later when doing the full
    // document parsing.
    let mut temp_parser = parser.clone();
    let mut state = PreprocessorState::new(&mut temp_parser);
    state.process_adoc_include(source, parser.primary_file_name.as_deref());

    (state.output, state.source_map)
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
                    // TODO: Use process_adoc_include or (TBD) depending on
                    // whether it's an Asciidoc file type.
                    self.process_adoc_include(&include_text, Some(&target));

                    // Re-report the including file if there's more content.
                    has_reported_file = false;
                } else {
                    self.output_line_number += 1;
                    self.output.push_str(&format!(
                        "Unresolved directive in {file_name} - {line}\n",
                        file_name = file_name.unwrap_or("(root file)",),
                        line = line.data(),
                    ));
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

                self.output_line_number += 1;
                self.output.push_str(line.data());
                self.output.push('\n');
            }
        }

        self.include_depth -= 1;
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
    #![allow(clippy::unwrap_used)]

    use crate::{
        parser::{SourceLine, preprocessor::preprocess},
        tests::{fixtures::inline_file_handler::InlineFileHandler, prelude::*},
    };

    #[test]
    fn no_preprocessor_directives() {
        let source =
            "= Document Title\n\nThis is a simple document with no includes or conditionals.";
        let parser = Parser::default().with_primary_file_name("test.adoc");

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "= Document Title\n\nUnresolved directive in main.adoc - include::missing.adoc[]\n\nMore content.\n"
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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
        let parser = Parser::default().with_primary_file_name("main.adoc");

        let (processed_source, source_map) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            "= Document Title\n\nUnresolved directive in main.adoc - include::missing.adoc[]\n\nMore content.\n"
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
    fn multiple_includes_same_line() {
        let source = "include::part1.adoc[] include::part2.adoc[]";

        let handler = InlineFileHandler::from_pairs([
            ("part1.adoc", "First part"),
            ("part2.adoc", "Second part"),
        ]);

        let parser = Parser::default()
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map) = preprocess(source, &parser);

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
