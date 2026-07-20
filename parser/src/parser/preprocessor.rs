use std::{borrow::Cow, sync::LazyLock};

use regex::{Captures, Regex, Replacer};

use crate::{
    HasSpan, Parser, SafeMode, Span,
    attributes::{Attrlist, AttrlistContext},
    content::AttributeMissing,
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
/// The fourth returned value lists the AsciiDoc files this pass included, each
/// paired with whether it was included in full; see the `includes` field of
/// [`PreprocessorState`]. `Parser::parse_deferred` folds these into the
/// document's [`Catalog`](crate::document::Catalog).
///
/// [include file]: https://docs.asciidoctor.org/asciidoc/latest/directives/include/
/// [conditional]: https://docs.asciidoctor.org/asciidoc/latest/directives/conditionals/
pub(crate) fn preprocess(
    source: &str,
    parser: &Parser,
) -> (String, SourceMap, Vec<DeferredWarning>, Vec<(String, bool)>) {
    preprocess_with_initial_file_name(source, parser, parser.primary_file_name.as_deref())
}

/// Like [`preprocess`], but treats `initial_file_name` (rather than the
/// parser's `primary_file_name`) as the file the top-level `source` came from.
///
/// This is used to preprocess the content of an AsciiDoc table cell, which the
/// cell reached from some enclosing file: naming that file lets an unresolved
/// `include::` directive inside the cell report the correct originating file in
/// its "Unresolved directive in …" replacement, matching Asciidoctor.
pub(crate) fn preprocess_with_initial_file_name(
    source: &str,
    parser: &Parser,
    initial_file_name: Option<&str>,
) -> (String, SourceMap, Vec<DeferredWarning>, Vec<(String, bool)>) {
    // Short-circuit if the original source document has no pre-processor
    // directives. `if` covers `ifdef`/`ifndef`/`ifeval`; `endif` is checked
    // separately because it does not share that prefix, and a stray `endif`
    // (with no opening conditional) is itself a directive that must be
    // processed — otherwise it would be emitted as literal content and its
    // unmatched-directive diagnostic would be lost.
    if !source.starts_with("include::")
        && !source.starts_with("if")
        && !source.starts_with("endif::")
        && !source.starts_with("\\if")
        && !source.starts_with("\\endif::")
        && !source.contains("\ninclude::")
        && !source.contains("\nif")
        && !source.contains("\nendif::")
        && !source.contains("\n\\if")
        && !source.contains("\n\\endif::")
        && !source.starts_with("\\include::")
        && !source.contains("\n\\include::")
        && initial_file_name.is_none()
    {
        return (source.to_owned(), SourceMap::default(), vec![], vec![]);
    }

    // We use a temporary clone of the parser to track document attribute values
    // while parsing. These get recalculated again later when doing the full
    // document parsing.
    let mut temp_parser = parser.clone();
    let mut state = PreprocessorState::new(&mut temp_parser);
    state.process_adoc_include(source, initial_file_name);

    // Any conditional directive still open once the whole source has been
    // processed was never closed by a matching `endif`.
    state.emit_unterminated_conditional_warnings();

    (
        state.output,
        state.source_map,
        state.warnings,
        state.includes,
    )
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

    /// AsciiDoc files included by directives written in the outermost document,
    /// in the order the `include::` directives were processed. Each entry pairs
    /// the include target (relative to the outermost document, AsciiDoc
    /// extension removed) with whether that directive merged the file *in full*
    /// (`true`) or only a `lines`/`tag(s)` portion of it (`false`); the same
    /// file may appear more than once. `Parser::parse_deferred` replays these
    /// through
    /// [`Catalog::register_include`](crate::document::Catalog::register_include),
    /// which resolves the full/partial value (a full include wins), so an
    /// inter-document cross reference to an included file can collapse to a
    /// same-document one.
    ///
    /// A directive in a *nested* include (depth 2 and below) is not recorded:
    /// its target is relative to the file containing it, not the outermost
    /// document, so registering it as written could falsely collapse a
    /// root-relative xref that names a different file.
    includes: Vec<(String, bool)>,

    /// The include-depth limit currently in effect, or `None` when
    /// `max-include-depth` is 0 — which disables the include directive
    /// entirely. See [`MaxIncludeDepth`].
    max_include_depth: Option<MaxIncludeDepth>,

    /// Stack of open conditional preprocessor directives (`ifdef`, `ifndef`,
    /// `ifeval`). Each entry records whether the lines it encloses are
    /// currently being skipped. See [`process_conditional_directive`].
    ///
    /// [`process_conditional_directive`]: Self::process_conditional_directive
    conditional_stack: Vec<Conditional>,
}

/// A single open block-form conditional preprocessor directive.
#[derive(Debug)]
struct Conditional {
    /// The directive target used to match a named `endif`. `ifdef`/`ifndef`
    /// store their attribute expression here; `ifeval` stores `None` (it can
    /// only be closed by an anonymous `endif::[]`).
    target: Option<String>,

    /// `true` if the lines enclosed by this directive are being discarded. This
    /// is cumulative: a conditional nested inside a skipped region is itself
    /// skipping, regardless of its own condition.
    skipping: bool,

    /// The opening directive as written (e.g. `ifdef::on-quest[]`), used to
    /// report it if it is never closed. See
    /// [`emit_unterminated_conditional_warnings`].
    ///
    /// [`emit_unterminated_conditional_warnings`]: PreprocessorState::emit_unterminated_conditional_warnings
    directive_text: String,

    /// The originating file and 1-based line of the opening directive, used to
    /// locate an "unterminated" warning at the directive's own line.
    file_name: Option<String>,
    source_line: usize,
}

/// The include-depth limit in effect, mirroring Asciidoctor's `@maxdepth`
/// state. Depths count the number of open includes: a directive in the root
/// file is at depth 0, one in a file it includes is at depth 1, and so on.
#[derive(Clone, Copy, Debug)]
struct MaxIncludeDepth {
    /// The absolute limit set by the `max-include-depth` attribute. A `depth`
    /// attribute on an include directive can never raise the effective limit
    /// above this.
    abs: usize,

    /// The depth at which further include directives are refused, compared
    /// against the depth of the file containing the directive. Initially equal
    /// to [`abs`](Self::abs); an include directive's `depth` attribute lowers
    /// it for the span of that include.
    curr: usize,

    /// The limit relative to the file that established it, reported in the
    /// "maximum include depth of N exceeded" diagnostic (matching
    /// Asciidoctor, which reports the requested relative depth rather than
    /// the absolute nesting level).
    rel: usize,
}

impl<'p> PreprocessorState<'p> {
    fn new(parser: &'p mut Parser) -> Self {
        // Asciidoctor reads `max-include-depth` once, when the reader is
        // constructed, so the value in effect at the start of preprocessing
        // governs the entire pass. (The attribute is API-only — see
        // `built_in_attrs.rs` — so the document cannot change it anyway.) The
        // value is coerced as Ruby's `to_i` would; a non-positive result
        // disables the include directive entirely.
        let max_include_depth = match parser.attribute_value("max-include-depth") {
            InterpretedValue::Value(value) => ruby_to_i(&value),
            // Set with an empty value coerces to 0 (disabled); unset falls
            // back to Asciidoctor's default of 64.
            InterpretedValue::Set => 0,
            InterpretedValue::Unset => 64,
        };

        // A positive value too large for `usize` (possible on 32-bit targets)
        // saturates to an effectively unlimited depth rather than failing the
        // conversion, which would otherwise be mistaken for the "disabled"
        // sentinel. (Ruby's integers are unbounded, so Asciidoctor simply
        // honors such a value as a very large limit.)
        let max_include_depth = (max_include_depth > 0).then(|| {
            let depth = usize::try_from(max_include_depth).unwrap_or(usize::MAX);
            MaxIncludeDepth {
                abs: depth,
                curr: depth,
                rel: depth,
            }
        });

        Self {
            parser,
            in_document_header: true,
            can_have_attribute: true,
            include_depth: 0,
            output_line_number: 1,
            output: String::new(),
            source_map: SourceMap::default(),
            warnings: vec![],
            includes: vec![],
            max_include_depth,
            conditional_stack: vec![],
        }
    }

    /// Returns `true` if the preprocessor is currently discarding lines because
    /// it is inside a conditional directive whose condition evaluated to false.
    fn skipping(&self) -> bool {
        self.conditional_stack.last().is_some_and(|c| c.skipping)
    }

    fn process_adoc_include(&mut self, source: &str, file_name: Option<&str>) {
        self.include_depth += 1;

        let mut has_reported_file = file_name.is_none();
        let mut source_span = Span::new(source);

        // Comment-block tracking. Asciidoctor's `PreprocessorReader` never
        // processes preprocessor directives inside a comment block: the parser
        // reads the block's content with line processing disabled. This crate
        // preprocesses in a separate pass, so it tracks that state here. See
        // issue #810.
        //
        // `comment_block_delimiter` is the closing delimiter of the comment
        // block currently open (a `////` run, or the `--` of a `[comment]` open
        // block); `in_comment_paragraph` is set while inside the raw portion of
        // a `[comment]` paragraph; `after_comment_style` records that the line
        // just emitted was a `[comment]` block-attribute line.
        let mut comment_block_delimiter: Option<String> = None;
        let mut in_comment_paragraph = false;
        let mut after_comment_style = false;

        while !source_span.is_empty() {
            let original_source = source_span;

            let MatchedItem { item: line, after } = source_span.take_line();
            source_span = after;

            let source_line_number = line.line();

            // Inside a comment block, every line is raw: emit it verbatim, with
            // no directive or include processing, until the closing delimiter.
            if let Some(delimiter) = &comment_block_delimiter {
                let closes = line.data() == delimiter;
                self.emit_line(
                    line.data(),
                    file_name,
                    source_line_number,
                    &mut has_reported_file,
                );
                if closes {
                    comment_block_delimiter = None;
                }
                continue;
            }

            // The lines of a `[comment]` paragraph after its first are likewise
            // raw, up to the blank line that ends the paragraph. (Its first line
            // is still processed, matching Asciidoctor's one-line look-ahead: by
            // the time a paragraph is recognized as a comment, the reader has
            // already visited that line.)
            if in_comment_paragraph {
                if line.data().is_empty() {
                    in_comment_paragraph = false;
                }
                self.emit_line(
                    line.data(),
                    file_name,
                    source_line_number,
                    &mut has_reported_file,
                );
                continue;
            }

            // Whether the line just emitted was a `[comment]` block-attribute
            // line (consumed below when classifying the block it introduces).
            let was_comment_style = std::mem::take(&mut after_comment_style);

            // Conditional preprocessor directives (`ifdef`, `ifndef`, `ifeval`,
            // `endif`) are handled before anything else so they take effect even
            // while a surrounding conditional is skipping (the nesting still has
            // to be tracked to balance the stack).
            if has_conditional_prefix(line.data())
                && let Some(caps) = CONDITIONAL_DIRECTIVE.captures(line.data())
            {
                // A directive line produces no output of its own, so the next
                // emitted line must re-anchor the source map (its original line
                // number no longer matches the output line number).
                has_reported_file = false;

                if caps.get(1).is_some() {
                    // Escaped directive (e.g. `\ifdef::foo[]`): not processed.
                    // The leading backslash is stripped and the remainder is
                    // emitted literally, matching Asciidoctor — unless we're
                    // skipping, in which case it's discarded like any other line.
                    if !self.skipping() {
                        self.emit_line(
                            &line.data()[1..],
                            file_name,
                            source_line_number,
                            &mut has_reported_file,
                        );
                    }
                } else {
                    self.process_conditional_directive(
                        &caps[2],
                        caps.get(3).map_or("", |m| m.as_str()),
                        caps.get(4).map_or("", |m| m.as_str()),
                        file_name,
                        source_line_number,
                        &mut has_reported_file,
                    );
                }

                continue;
            }

            // While skipping (inside a conditional whose condition was false),
            // discard every non-directive line.
            if self.skipping() {
                has_reported_file = false;
                continue;
            }

            // A `////` line (four or more slashes, nothing else) opens a comment
            // block. Its content is raw, so it is emitted verbatim and no
            // directive within it is processed until the matching closing
            // delimiter. See issue #810.
            if is_comment_block_delimiter(line.data()) {
                comment_block_delimiter = Some(line.data().to_owned());
                self.emit_line(
                    line.data(),
                    file_name,
                    source_line_number,
                    &mut has_reported_file,
                );
                continue;
            }

            // A `[comment]` block-attribute line turns the block it introduces
            // into a comment. When that block is an open block (`--`), its
            // content is raw up to the closing `--`; otherwise it is a comment
            // paragraph, whose lines after the first are raw (see above).
            if was_comment_style {
                if line.data() == "--" {
                    comment_block_delimiter = Some("--".to_owned());
                    self.emit_line(
                        line.data(),
                        file_name,
                        source_line_number,
                        &mut has_reported_file,
                    );
                    continue;
                }

                if !line.data().is_empty() {
                    in_comment_paragraph = true;
                }
            }

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
                // Asciidoctor substitutes attributes into an include target
                // using the `attribute-missing` policy in effect, except that
                // `warn` is mapped to `drop-line`: a warning here names the
                // whole directive, not the individual reference. Under either
                // of those policies a reference to a missing attribute empties
                // the entire target, and the directive is dropped before the
                // include file handler is ever consulted. See issue #776.
                let attribute_missing = AttributeMissing::from_parser(self.parser);

                let missing_policy = match attribute_missing {
                    AttributeMissing::Skip => MissingAttribute::KeepLiteral,
                    AttributeMissing::Drop => MissingAttribute::Drop,
                    AttributeMissing::DropLine | AttributeMissing::Warn => {
                        MissingAttribute::DropLine
                    }
                };

                let (target, missing_reference) =
                    self.substitute_attributes_tracking(&caps[1], missing_policy);

                if missing_reference
                    && matches!(
                        attribute_missing,
                        AttributeMissing::DropLine | AttributeMissing::Warn
                    )
                {
                    // Under `drop-line` (and for an include marked
                    // `opts=optional`) the directive line is removed with no
                    // replacement text. Asciidoctor logs this at INFO level;
                    // this crate has no INFO channel, so — as everywhere else
                    // `drop-line` applies — the line is dropped silently.
                    // Re-anchor the source map so the lines that follow map
                    // back to their correct original line numbers.
                    if attribute_missing == AttributeMissing::DropLine
                        || parse_attrlist(&caps, self.parser).has_option("optional")
                    {
                        has_reported_file = false;
                        continue;
                    }

                    // Under `warn` the directive is replaced by an "Unresolved
                    // directive" message, as it is for a target that could not
                    // be resolved, and a warning naming the whole directive is
                    // recorded.
                    self.emit_unresolved_directive(
                        line.data(),
                        WarningType::IncludeDroppedDueToMissingAttribute(line.data().to_owned()),
                        file_name,
                        source_line_number,
                        &mut has_reported_file,
                    );

                    continue;
                }

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

                    // A target containing a space would break the link macro,
                    // so it is wrapped in a `pass:c[…]` macro (matching
                    // Asciidoctor).
                    let replacement = if target.contains(' ') {
                        format!("link:pass:c[{target}][role=include]")
                    } else {
                        format!("link:{target}[role=include]")
                    };
                    self.output_line_number += 1;
                    self.output.push_str(&replacement);
                    self.output.push('\n');

                    continue;
                }

                // `max-include-depth=0` disables the include directive
                // entirely: the directive line is left in the output verbatim,
                // with no diagnostic, and the include file handler is never
                // consulted (matching Asciidoctor).
                let Some(max_depth) = self.max_include_depth else {
                    self.emit_line(
                        line.data(),
                        file_name,
                        source_line_number,
                        &mut has_reported_file,
                    );
                    continue;
                };

                // When the file containing the directive already sits at the
                // maximum include depth, the directive is likewise left
                // verbatim, and a "maximum include depth exceeded" error is
                // recorded at the directive's own file and line (matching
                // Asciidoctor). `include_depth` counts the current file as 1,
                // so the containing file's depth — which the limit is compared
                // against — is `include_depth - 1`, making the depth-exceeded
                // condition `include_depth - 1 >= curr`, i.e.:
                if self.include_depth > max_depth.curr {
                    self.warnings.push(DeferredWarning {
                        offset: self.output.len(),
                        len: line.data().len(),
                        warning: WarningType::MaxIncludeDepthExceeded(max_depth.rel),
                        origin: None,
                    });

                    self.emit_line(
                        line.data(),
                        file_name,
                        source_line_number,
                        &mut has_reported_file,
                    );
                    continue;
                }

                let attrlist = parse_attrlist(&caps, self.parser);

                // A URI target is only honored when the URI read permission has
                // been granted (`allow-uri-read`). This is disabled by default,
                // so a URI include that is not permitted is treated as an
                // unresolved directive. (At `SafeMode::Secure` and above the
                // include was already converted to a link above; `allow-uri-read`
                // is meaningful only below that level.) See `include-uri.adoc`.
                if is_uri(&target) && !self.parser.is_attribute_set("allow-uri-read") {
                    self.emit_unresolved_directive(
                        line.data(),
                        WarningType::IncludeFileNotFound(target),
                        file_name,
                        source_line_number,
                        &mut has_reported_file,
                    );
                    continue;
                }

                if let Some(include_content) =
                    self.parser.include_file_handler.as_ref().and_then(|ifh| {
                        ifh.resolve_target(file_name, &target, &attrlist, self.parser)
                    })
                {
                    // Apply `lines`/`tag(s)` selection and `indent` normalization
                    // to the raw included content before it is merged, matching
                    // Asciidoctor. Any nested include/conditional directives in an
                    // AsciiDoc include are therefore interpreted only on the
                    // selected, re-indented lines.
                    let (selected, tag_diagnostics) =
                        select_included_lines(include_content.content(), &attrlist);
                    let selected = reindent_included_lines(selected, &attrlist, self.parser);

                    // A malformed or unmatched tag directive (or a requested tag
                    // that was never found) is reported against the include
                    // directive's own cursor.
                    self.emit_tag_filter_warnings(&tag_diagnostics, file_name, source_line_number);

                    // The parser only handles UTF-8 content, so an `encoding`
                    // attribute requesting any other encoding cannot be honored
                    // by the parser itself; record a warning (emitted below, once
                    // the offset of the included content is known). See
                    // `include.adoc`. A handler that transcodes the content to
                    // UTF-8 itself signals this via `IncludeContent::transcoded`,
                    // in which case the encoding has been honored and no warning
                    // is recorded. See
                    // https://github.com/asciidoc-rs/asciidoc-parser/issues/611.
                    let non_utf8_encoding = (!include_content.encoding_handled())
                        .then(|| {
                            attrlist
                                .named_attribute("encoding")
                                .map(|a| a.value())
                                .filter(|v| !is_utf8_encoding(v))
                        })
                        .flatten();

                    // `leveloffset` wraps the included content in `:leveloffset:`
                    // attribute entries: the offset is applied to the included
                    // content and reset afterward (see
                    // `include-with-leveloffset.adoc`). The running `leveloffset`
                    // document attribute is applied to section levels during
                    // parsing (see `SectionBlock::parse` and
                    // `Parser::level_offset`), so this wrapping shifts the
                    // effective heading levels of the included content.
                    let leveloffset = attrlist
                        .named_attribute("leveloffset")
                        .map(|a| a.value())
                        .filter(|v| !v.is_empty());

                    // Capture the restore value *before* processing the include:
                    // an included AsciiDoc file may itself set `:leveloffset:`,
                    // which would mutate the running attribute state, so reading it
                    // afterward would restore the included file's value rather than
                    // the one in effect before the include.
                    let restore_leveloffset = leveloffset.map(|offset| {
                        let restore = match self.parser.attribute_value("leveloffset") {
                            InterpretedValue::Value(v) if !v.is_empty() => {
                                format!(":leveloffset: {v}")
                            }
                            _ => ":leveloffset!:".to_string(),
                        };
                        self.emit_line(
                            &format!(":leveloffset: {offset}"),
                            file_name,
                            source_line_number,
                            &mut has_reported_file,
                        );
                        self.emit_line("", file_name, source_line_number, &mut has_reported_file);
                        restore
                    });

                    let content_start = self.output.len();

                    if is_asciidoc_file(&target) {
                        // Register the included AsciiDoc file so an
                        // inter-document cross reference whose target names it
                        // can later collapse to a same-document reference (its
                        // anchors are now part of this document). A `lines` or
                        // partial `tag(s)` selection records a *partial* include,
                        // which does not collapse the reference. A file
                        // included both fully and partially resolves to full;
                        // that merge is applied by
                        // [`Catalog::register_include`] when these entries are
                        // replayed into the catalog.
                        //
                        // Only a directive written in the *outermost* document
                        // (depth 1) is recorded: its target is already in the
                        // coordinate system an inter-document xref target uses.
                        // A nested include's target is relative to the file
                        // containing it, so registering it as written could
                        // collide with — and falsely collapse — a root-relative
                        // xref that names a different file. A nested include is
                        // therefore not recorded at all: an xref to it keeps
                        // its ordinary inter-document destination.
                        //
                        // [`Catalog::register_include`]: crate::document::Catalog::register_include
                        if self.include_depth == 1 {
                            let full = is_full_include(&attrlist);
                            self.includes
                                .push((include_catalog_key(&target).to_string(), full));
                        }

                        // The directive's `depth` attribute lowers the maximum
                        // include depth while the included file (and anything
                        // it includes) is processed; the previous limit is
                        // restored once the include has been merged. A positive
                        // value permits that many more levels below the
                        // included file, clamped to the absolute
                        // `max-include-depth` limit; zero (or a value that
                        // coerces to zero) permits none. `include_depth` here
                        // is the containing file's depth plus one — i.e. the
                        // depth of the included file itself.
                        let saved_max_depth = self.max_include_depth;

                        if let Some(depth_attr) = attrlist.named_attribute("depth")
                            && let Some(max_depth) = self.max_include_depth.as_mut()
                        {
                            let rel = ruby_to_i(depth_attr.value());
                            if rel > 0 {
                                // A request too large for `usize` (possible on
                                // 32-bit targets) saturates rather than
                                // wrapping into a restrictive value; the clamp
                                // below then reduces it to the absolute limit.
                                let mut rel = usize::try_from(rel).unwrap_or(usize::MAX);
                                let mut curr = self.include_depth.saturating_add(rel);
                                if curr > max_depth.abs {
                                    curr = max_depth.abs;
                                    rel = max_depth.abs;
                                }
                                max_depth.curr = curr;
                                max_depth.rel = rel;
                            } else {
                                max_depth.curr = self.include_depth;
                                max_depth.rel = 0;
                            }
                        }

                        // AsciiDoc files are run through the preprocessor, so the
                        // include (and other) directives they contain are
                        // interpreted.
                        self.process_adoc_include(&selected, Some(&target));

                        self.max_include_depth = saved_max_depth;
                    } else {
                        // Non-AsciiDoc files are merged verbatim; the preprocessor
                        // does not interpret any AsciiDoc directives within them
                        // (matching Asciidoctor).
                        self.process_nonadoc_include(&selected, Some(&target));
                    }

                    if let Some(encoding) = non_utf8_encoding {
                        // Point the warning at the first line of the included
                        // content (the directive line itself is not present in the
                        // output once it has been expanded).
                        let len = self.output[content_start..]
                            .find('\n')
                            .unwrap_or(self.output.len() - content_start);
                        self.warnings.push(DeferredWarning {
                            offset: content_start,
                            len,
                            warning: WarningType::NonUtf8IncludeEncoding(encoding.to_string()),
                            origin: None,
                        });
                    }

                    if let Some(restore) = restore_leveloffset {
                        // Reset the level offset to whatever was in effect before
                        // the include (unset unless a `:leveloffset:` was active).
                        self.emit_line("", file_name, source_line_number, &mut has_reported_file);
                        self.emit_line(
                            &restore,
                            file_name,
                            source_line_number,
                            &mut has_reported_file,
                        );
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
                    // an "Unresolved directive" message and record a warning.
                    self.emit_unresolved_directive(
                        line.data(),
                        WarningType::IncludeFileNotFound(target),
                        file_name,
                        source_line_number,
                        &mut has_reported_file,
                    );
                }
            } else {
                // If none of the above apply, add the line to output.
                //
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

                self.emit_line(
                    line_text,
                    file_name,
                    source_line_number,
                    &mut has_reported_file,
                );

                // Remember a `[comment]` block-attribute line so the next
                // iteration can classify the block it introduces as a comment.
                if line_text == "[comment]" {
                    after_comment_style = true;
                }
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
    /// patterns with their corresponding values from the parser. A reference to
    /// an unset attribute is handled per `missing`.
    fn substitute_attributes(&self, input: &str, missing: MissingAttribute) -> String {
        self.substitute_attributes_tracking(input, missing).0
    }

    /// Apply attribute substitution as [`substitute_attributes`] does, also
    /// reporting whether a (non-escaped) reference to an unset attribute was
    /// found. In [`MissingAttribute::DropLine`] mode the substituted text is
    /// meaningless once that flag is set — the caller drops the line it came
    /// from.
    ///
    /// [`substitute_attributes`]: Self::substitute_attributes
    fn substitute_attributes_tracking(
        &self,
        input: &str,
        missing: MissingAttribute,
    ) -> (String, bool) {
        if !input.contains('{') {
            return (input.to_string(), false);
        }

        #[derive(Debug)]
        struct AttributeReplacer<'p> {
            parser: &'p Parser,
            missing: MissingAttribute,

            /// Set to `true` when a (non-escaped) reference to an unset
            /// attribute is encountered.
            missing_reference: bool,
        }

        impl Replacer for AttributeReplacer<'_> {
            fn replace_append(&mut self, caps: &regex::Captures<'_>, dest: &mut String) {
                let attr_name = &caps[1];

                // An escaped reference (e.g. `\{id}`) is emitted literally with
                // the escaping backslash removed, whether or not the attribute
                // is set. This mirrors the content-substitution path and
                // Asciidoctor's `sub_attributes` (see issue #667).
                if caps[0].starts_with('\\') {
                    dest.push_str(&caps[0][1..]);
                    return;
                }

                if !self.parser.has_attribute(attr_name) {
                    self.missing_reference = true;

                    if matches!(self.missing, MissingAttribute::KeepLiteral) {
                        dest.push_str(&caps[0]);
                    }
                    return;
                }

                if let InterpretedValue::Value(value) = self.parser.attribute_value(attr_name) {
                    dest.push_str(value.as_ref());
                }
            }
        }

        let result: Cow<'_, str> = input.into();

        let mut replacer = AttributeReplacer {
            parser: self.parser,
            missing,
            missing_reference: false,
        };

        let replaced = ATTRIBUTE_REFERENCE.replace_all(&result, replacer.by_ref());

        let text = match replaced {
            Cow::Owned(new_result) => new_result,
            Cow::Borrowed(_) => input.to_string(),
        };

        (text, replacer.missing_reference)
    }

    /// Emit a single line of text to the output, updating the source map and
    /// document-header tracking state exactly as the plain-line branch of
    /// [`process_adoc_include`] does.
    ///
    /// [`process_adoc_include`]: Self::process_adoc_include
    fn emit_line(
        &mut self,
        text: &str,
        file_name: Option<&str>,
        source_line_number: usize,
        has_reported_file: &mut bool,
    ) {
        if !*has_reported_file {
            *has_reported_file = true;
            self.source_map.append(
                self.output_line_number,
                SourceLine(to_owned(file_name), source_line_number),
            );
        }

        if text.is_empty() {
            self.in_document_header = false;
            self.can_have_attribute = true;
        } else if !self.in_document_header {
            self.can_have_attribute = false;
        }

        self.output_line_number += 1;
        self.output.push_str(text);
        self.output.push('\n');
    }

    /// Replace an include directive that could not be resolved with an
    /// "Unresolved directive" message (as Asciidoctor does) and record
    /// `warning` pointing at that message. The warning is located by byte
    /// offset because the output it refers to is not yet owned;
    /// `Document::parse` reconstitutes it into a spanned [`Warning`].
    ///
    /// [`Warning`]: crate::warnings::Warning
    fn emit_unresolved_directive(
        &mut self,
        directive_line: &str,
        warning: WarningType,
        file_name: Option<&str>,
        source_line_number: usize,
        has_reported_file: &mut bool,
    ) {
        if !*has_reported_file {
            *has_reported_file = true;
            self.source_map.append(
                self.output_line_number,
                SourceLine(to_owned(file_name), source_line_number),
            );
        }

        let replacement = format!(
            "Unresolved directive in {file_name} - {directive_line}",
            file_name = file_name.unwrap_or("(root file)"),
        );

        self.warnings.push(DeferredWarning {
            offset: self.output.len(),
            len: replacement.len(),
            warning,
            origin: None,
        });

        self.output_line_number += 1;
        self.output.push_str(&replacement);
        self.output.push('\n');
    }

    /// Process a conditional preprocessor directive (`ifdef`, `ifndef`,
    /// `ifeval`, or `endif`).
    ///
    /// `keyword` is the directive name, `target` is the text before the `[`
    /// (attribute expression for `ifdef`/`ifndef`, empty for `ifeval`), and
    /// `content` is the text inside the brackets (single-line content for
    /// `ifdef`/`ifndef`, the expression for `ifeval`).
    ///
    /// See the [conditionals] documentation.
    ///
    /// [conditionals]: https://docs.asciidoctor.org/asciidoc/latest/directives/conditionals/
    fn process_conditional_directive(
        &mut self,
        keyword: &str,
        target: &str,
        content: &str,
        file_name: Option<&str>,
        source_line_number: usize,
        has_reported_file: &mut bool,
    ) {
        let already_skipping = self.skipping();

        if keyword == "endif" {
            // `endif::[]` closes the most recently opened conditional;
            // `endif::name[]` must match that conditional's target. An `endif`
            // with non-empty brackets (e.g. `endif::name[text]`) is malformed
            // and closes nothing; a mismatched or unmatched `endif` likewise
            // closes nothing. Asciidoctor logs an error in each case (but stays
            // silent while an enclosing conditional is already skipping — the
            // stray `endif` is just discarded along with the skipped region).
            if !content.is_empty() {
                if !already_skipping {
                    self.emit_conditional_warning(
                        WarningType::MalformedConditionalDirective(
                            "text not permitted".to_owned(),
                            directive_text(keyword, target, content),
                        ),
                        file_name,
                        source_line_number,
                    );
                }
                return;
            }

            match self.conditional_stack.last() {
                Some(top) if target.is_empty() || top.target.as_deref() == Some(target) => {
                    self.conditional_stack.pop();
                }
                Some(_) => {
                    if !already_skipping {
                        self.emit_conditional_warning(
                            WarningType::MismatchedConditionalDirective(directive_text(
                                keyword, target, content,
                            )),
                            file_name,
                            source_line_number,
                        );
                    }
                }
                None => {
                    // The stack is empty, so nothing is skipping: always warn.
                    self.emit_conditional_warning(
                        WarningType::UnmatchedConditionalDirective(directive_text(
                            keyword, target, content,
                        )),
                        file_name,
                        source_line_number,
                    );
                }
            }
            return;
        }

        if keyword == "ifeval" {
            // `ifeval` has no single-line or long-form variant, its target must
            // be empty, and its bracketed expression is required and must be a
            // valid comparison. A malformed `ifeval` is dropped (it opens no
            // conditional and does not enclose the lines that follow), with an
            // error logged unless an enclosing conditional is already skipping.
            let malformed_reason = if !target.is_empty() {
                Some("target not permitted")
            } else if content.trim().is_empty() {
                Some("missing expression")
            } else if !IFEVAL_EXPRESSION.is_match(content.trim()) {
                Some("invalid expression")
            } else {
                None
            };

            if let Some(reason) = malformed_reason {
                if !already_skipping {
                    self.emit_conditional_warning(
                        WarningType::MalformedConditionalDirective(
                            reason.to_owned(),
                            directive_text(keyword, target, content),
                        ),
                        file_name,
                        source_line_number,
                    );
                }
                return;
            }

            let include = !already_skipping && self.eval_ifeval(content);
            self.conditional_stack.push(Conditional {
                target: None,
                skipping: already_skipping || !include,
                directive_text: directive_text(keyword, target, content),
                file_name: to_owned(file_name),
                source_line: source_line_number,
            });
            return;
        }

        // `ifdef` / `ifndef`.
        if target.is_empty() {
            // Malformed: a target (attribute name) is required. Dropped, with an
            // error logged unless already skipping.
            if !already_skipping {
                self.emit_conditional_warning(
                    WarningType::MalformedConditionalDirective(
                        "missing target".to_owned(),
                        directive_text(keyword, target, content),
                    ),
                    file_name,
                    source_line_number,
                );
            }
            return;
        }

        if content.is_empty() {
            // Block form: skip the enclosed lines unless the condition holds
            // (and never include them while already skipping).
            let skipping = already_skipping || !self.eval_ifdef(keyword, target);
            self.conditional_stack.push(Conditional {
                target: Some(target.to_owned()),
                skipping,
                directive_text: directive_text(keyword, target, content),
                file_name: to_owned(file_name),
                source_line: source_line_number,
            });
        } else if !already_skipping && self.eval_ifdef(keyword, target) {
            // Single-line form: the bracketed content is included in place (with
            // no `endif`) when the condition holds.
            self.process_single_line_content(
                content,
                file_name,
                source_line_number,
                has_reported_file,
            );
        }
    }

    /// Record a warning for a conditional preprocessor directive.
    ///
    /// A conditional directive produces no output of its own, so there is no
    /// output span to resolve the warning's location against. The directive's
    /// originating file and line are therefore recorded on the warning directly
    /// (via [`DeferredWarning::origin`]); the byte-offset span is a zero-length
    /// best-effort anchor at the current output position.
    ///
    /// [`DeferredWarning::origin`]: crate::parser::DeferredWarning::origin
    fn emit_conditional_warning(
        &mut self,
        warning: WarningType,
        file_name: Option<&str>,
        source_line_number: usize,
    ) {
        self.warnings.push(DeferredWarning {
            offset: self.output.len(),
            len: 0,
            warning,
            origin: Some(SourceLine(to_owned(file_name), source_line_number)),
        });
    }

    /// Record a warning for each tag-filter diagnostic raised while resolving
    /// an include directive's `tag(s)` selection.
    ///
    /// Each is located at the include directive's own cursor (its file and
    /// line), matching Asciidoctor's `include_location`. The byte-offset span
    /// is a zero-length best-effort anchor at the current output position.
    fn emit_tag_filter_warnings(
        &mut self,
        diagnostics: &[TagFilterDiagnostic],
        file_name: Option<&str>,
        source_line_number: usize,
    ) {
        for diagnostic in diagnostics {
            let warning = match diagnostic {
                TagFilterDiagnostic::NotFound(names) => {
                    let word = if names.len() > 1 { "tags" } else { "tag" };
                    WarningType::IncludeTagNotFound(format!("{word} '{}'", names.join(", ")))
                }
                TagFilterDiagnostic::Unclosed(name) => {
                    WarningType::IncludeTagUnclosed(format!("'{name}'"))
                }
                TagFilterDiagnostic::MismatchedEnd { expected, found } => {
                    WarningType::IncludeTagMismatchedEnd(
                        format!("'{expected}'"),
                        format!("'{found}'"),
                    )
                }
                TagFilterDiagnostic::UnexpectedEnd(name) => {
                    WarningType::IncludeTagUnexpectedEnd(format!("'{name}'"))
                }
            };

            self.warnings.push(DeferredWarning {
                offset: self.output.len(),
                len: 0,
                warning,
                origin: Some(SourceLine(to_owned(file_name), source_line_number)),
            });
        }
    }

    /// Emit an "unterminated" warning for each conditional directive still open
    /// at the end of preprocessing (i.e. never closed by a matching `endif`).
    ///
    /// Directives are reported in the order they were opened, each located at
    /// its own opening line. Asciidoctor reports an unterminated conditional at
    /// the end of the reader by default, but at the opening directive's line
    /// when `sourcemap` is enabled; this crate always maintains a source map,
    /// so it always reports at the opening line.
    fn emit_unterminated_conditional_warnings(&mut self) {
        // Drain the stack so the directive text can be moved into each warning
        // without cloning; the state is discarded after this call.
        for conditional in std::mem::take(&mut self.conditional_stack) {
            self.warnings.push(DeferredWarning {
                offset: self.output.len(),
                len: 0,
                warning: WarningType::UnterminatedConditionalDirective(conditional.directive_text),
                origin: Some(SourceLine(conditional.file_name, conditional.source_line)),
            });
        }
    }

    /// Emit the bracketed content of a single-line `ifdef`/`ifndef` directive.
    ///
    /// The content is a single line. When it is an attribute entry (e.g.
    /// `ifdef::x[:foo: bar]`) it is applied to the running attribute state so
    /// that later directives and include targets observe it, matching how the
    /// main loop treats attribute entries; otherwise it is emitted verbatim and
    /// left for normal parsing (any `{attr}` references are resolved then).
    fn process_single_line_content(
        &mut self,
        content: &str,
        file_name: Option<&str>,
        source_line_number: usize,
        has_reported_file: &mut bool,
    ) {
        let can_have_attribute = self.can_have_attribute;
        let mut applied_attribute = false;

        if can_have_attribute
            && content.starts_with(':')
            && (content.ends_with(':') || content.contains(": "))
            && let Some(attr) = Attribute::parse(Span::new(content), self.parser)
        {
            let mut warnings: Vec<Warning> = vec![];
            self.parser
                .set_attribute_from_body(&attr.item, &mut warnings);
            applied_attribute = true;
        }

        self.emit_line(content, file_name, source_line_number, has_reported_file);

        // The main attribute-entry handler leaves `can_have_attribute` unchanged
        // so that consecutive attribute entries are all applied by the
        // preprocessor (and thus observed by later include targets); `emit_line`
        // would instead clear it for this non-empty line. Restore the invariant
        // when the single-line content was itself an attribute entry.
        if applied_attribute {
            self.can_have_attribute = can_have_attribute;
        }
    }

    /// Evaluate an `ifdef`/`ifndef` condition, returning `true` if the enclosed
    /// content should be included.
    ///
    /// Multiple attribute names may be combined with `,` (any is set — logical
    /// OR) or `+` (all are set — logical AND); the two combinators cannot be
    /// mixed. `ifndef` is the logical negation of `ifdef`.
    fn eval_ifdef(&self, keyword: &str, target: &str) -> bool {
        // Attribute names are case-insensitive: the parser stores them
        // lowercased, so the directive's target names are lowercased to match
        // (`ifdef::showScript[]` resolves the `showscript` attribute).
        let is_set = |name: &str| self.parser.is_attribute_set(name.to_lowercase());

        let defined = if target.contains(',') {
            target.split(',').any(is_set)
        } else if target.contains('+') {
            target.split('+').all(is_set)
        } else {
            is_set(target)
        };

        if keyword == "ifndef" {
            !defined
        } else {
            defined
        }
    }

    /// Evaluate an `ifeval` expression, returning `true` if the enclosed
    /// content should be included. A malformed expression (or one whose two
    /// sides cannot be compared) evaluates to false.
    fn eval_ifeval(&self, expr: &str) -> bool {
        let Some(caps) = IFEVAL_EXPRESSION.captures(expr.trim()) else {
            return false;
        };

        let lhs = self.resolve_expr_val(&caps[1]);
        let rhs = self.resolve_expr_val(&caps[3]);
        compare_values(&lhs, &caps[2], &rhs)
    }

    /// Resolve one side of an `ifeval` expression to a typed [`Value`].
    ///
    /// Attribute references are substituted first; a reference to an unset
    /// attribute resolves to the empty string, as in Asciidoctor. A value
    /// enclosed in single or double quotes is always a string; otherwise it is
    /// coerced per the documented rules (empty → nil, `true`/`false` →
    /// boolean, a value with a period → float, anything else → integer).
    fn resolve_expr_val(&self, raw: &str) -> Value {
        let raw = raw.trim();

        // A value wrapped in matching single or double quotes is always a string.
        let quoted_inner = raw
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| raw.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')));

        match quoted_inner {
            Some(inner) => Value::Str(self.substitute_attributes(inner, MissingAttribute::Drop)),
            None => coerce_unquoted(&self.substitute_attributes(raw, MissingAttribute::Drop)),
        }
    }
}

/// How attribute substitution treats a reference to an unset attribute.
#[derive(Clone, Copy, Debug)]
enum MissingAttribute {
    /// Leave the `{name}` reference in place unchanged, deferring to normal
    /// content parsing (and its `attribute-missing` handling) downstream.
    KeepLiteral,

    /// Resolve the reference to the empty string. This mirrors Asciidoctor's
    /// `attribute_missing: 'drop'` option, which its `ifeval` operand
    /// resolution always applies regardless of the `attribute-missing`
    /// document attribute (see issue #779).
    Drop,

    /// Discard the text the reference occurs in entirely. The substituted text
    /// is still produced (minus the reference), but the caller is expected to
    /// throw it away once
    /// [`substitute_attributes_tracking`](PreprocessorState::substitute_attributes_tracking)
    /// reports a missing reference. This mirrors Asciidoctor's
    /// `attribute_missing: 'drop-line'` option.
    DropLine,
}

/// A value that one side of an `ifeval` expression has been coerced to.
#[derive(Debug, PartialEq)]
enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Nil,
}

/// Coerce an unquoted `ifeval` operand to a typed [`Value`] per the documented
/// rules.
fn coerce_unquoted(s: &str) -> Value {
    if s.is_empty() {
        return Value::Nil;
    }

    match s {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }

    if s.chars().all(char::is_whitespace) {
        return Value::Str(" ".to_owned());
    }

    if s.contains('.') {
        Value::Float(ruby_to_f(s))
    } else {
        Value::Int(ruby_to_i(s))
    }
}

/// Parse the leading integer of a string, Ruby `String#to_i` style (a string
/// with no leading numeric portion yields `0`).
///
/// Ruby's integers are unbounded; a value beyond the range of `i64` saturates
/// to `i64::MIN`/`i64::MAX` (by sign) so that a very large magnitude is not
/// mistaken for 0.
fn ruby_to_i(s: &str) -> i64 {
    let mut digits = String::new();

    for (idx, ch) in s.trim().char_indices() {
        if (idx == 0 && (ch == '+' || ch == '-')) || ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            break;
        }
    }

    digits.parse().unwrap_or_else(|_| {
        if !digits.bytes().any(|b| b.is_ascii_digit()) {
            // No numeric portion at all (empty, or a bare `+`/`-`): 0, as
            // Ruby's `to_i` yields.
            0
        } else if digits.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

/// Parse the leading float of a string, Ruby `String#to_f` style (a string with
/// no leading numeric portion yields `0.0`).
fn ruby_to_f(s: &str) -> f64 {
    let s = s.trim();
    if let Ok(f) = s.parse::<f64>() {
        return f;
    }

    let mut digits = String::new();
    let mut seen_dot = false;

    for (idx, ch) in s.char_indices() {
        if (idx == 0 && (ch == '+' || ch == '-')) || ch.is_ascii_digit() {
            digits.push(ch);
        } else if ch == '.' && !seen_dot {
            seen_dot = true;
            digits.push(ch);
        } else {
            break;
        }
    }

    digits.parse().unwrap_or(0.0)
}

/// Compare two `ifeval` values with the given operator, following Ruby's
/// comparison semantics. Equality across value types is simply `false`; an
/// ordering comparison (`<`, `<=`, `>`, `>=`) between values that cannot be
/// ordered (e.g. a number and a string) fails and yields `false`.
fn compare_values(lhs: &Value, op: &str, rhs: &Value) -> bool {
    // `op` is always one of the six operators matched by `IFEVAL_EXPRESSION`.
    match op {
        "==" => values_equal(lhs, rhs),
        "!=" => !values_equal(lhs, rhs),
        _ => match ordering_of(lhs, rhs) {
            Some(ordering) => match op {
                "<" => ordering.is_lt(),
                "<=" => ordering.is_le(),
                ">" => ordering.is_gt(),
                // The remaining ordering operator is `>=`.
                _ => ordering.is_ge(),
            },
            None => false,
        },
    }
}

fn values_equal(lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => (*a as f64) == *b,
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Nil, Value::Nil) => true,
        _ => false,
    }
}

fn ordering_of(lhs: &Value, rhs: &Value) -> Option<std::cmp::Ordering> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
        (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
        (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

/// Returns `true` if `line` could be a conditional preprocessor directive,
/// gating the (more expensive) regex match. An optional leading backslash marks
/// an escaped directive.
fn has_conditional_prefix(line: &str) -> bool {
    let line = line.strip_prefix('\\').unwrap_or(line);
    line.starts_with("ifdef::")
        || line.starts_with("ifndef::")
        || line.starts_with("ifeval::")
        || line.starts_with("endif::")
}

fn to_owned(maybe_file_name: Option<&str>) -> Option<String> {
    maybe_file_name.map(|n| n.to_string())
}

/// Returns `true` if `line` is a `////` comment-block delimiter: a run of four
/// or more slashes and nothing else. A shorter run (`//`, `///`) is a line
/// comment, not a delimiter. This mirrors the comment case of
/// [`RawDelimitedBlock::is_valid_delimiter`](crate::blocks::RawDelimitedBlock).
fn is_comment_block_delimiter(line: &str) -> bool {
    line.len() >= 4 && line.bytes().all(|b| b == b'/')
}

/// Parse the attribute list of an `include::` directive from the directive's
/// [`INCLUDE_DIRECTIVE`] captures. Group 2 is the text between the brackets; a
/// directive with no bracketed text yields an empty attribute list.
fn parse_attrlist<'src>(caps: &Captures<'src>, parser: &Parser) -> Attrlist<'src> {
    caps.get(2)
        .map(|attrlist| {
            let span = Span::new(attrlist.as_str());

            Attrlist::parse(span, parser, AttrlistContext::Inline)
                .item
                .item
        })
        .unwrap_or_default()
}

/// Reconstruct a conditional preprocessor directive as written, for use in a
/// diagnostic message (e.g. `endif::on-quest[]`, `ifeval::[1 | 2]`).
fn directive_text(keyword: &str, target: &str, content: &str) -> String {
    format!("{keyword}::{target}[{content}]")
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

/// The [`Catalog`](crate::document::Catalog) include-registry key for an
/// AsciiDoc include `target`: the target with its AsciiDoc file extension
/// removed, matching the path an inter-document xref target interprets to (see
/// [`interpret_xref_target`](crate::content)). `target` must name an AsciiDoc
/// file (see [`is_asciidoc_file`]).
///
/// The key is the target as written in the `include::` directive. Only
/// directives written in the outermost document are registered (see the
/// `includes` field of [`PreprocessorState`]), so the key is always relative to
/// that document — the coordinate system an inter-document xref target uses.
fn include_catalog_key(target: &str) -> &str {
    // `target` names an AsciiDoc file, so its final `.`-delimited segment is the
    // extension to strip; only the trailing extension is removed, so a path that
    // contains a period elsewhere (`using-.net-web-services.adoc`) keeps it.
    match target.rsplit_once('.') {
        Some((stem, _ext)) if !stem.is_empty() => stem,
        _ => target,
    }
}

/// Reports whether an `include::` directive with the given attributes merges
/// the file *in full*, as opposed to selecting only a portion of it.
///
/// A `lines` selection is always partial. A `tag(s)` selection is partial too,
/// except for the `**` wildcard, which selects every line of the file (both
/// tagged and untagged regions) and so is a full include — matching
/// Asciidoctor's `catalog[:includes]` bookkeeping.
fn is_full_include(attrlist: &Attrlist<'_>) -> bool {
    if attrlist
        .named_attribute("lines")
        .map(|a| a.value())
        .is_some_and(|v| !v.is_empty())
    {
        return false;
    }

    match attrlist
        .named_attribute("tags")
        .or_else(|| attrlist.named_attribute("tag"))
        .map(|a| a.value())
        .filter(|v| !v.is_empty())
    {
        // `tags=**` selects the whole file; any other selection is partial.
        Some(tags) => tags.trim() == "**",
        None => true,
    }
}

/// Returns `true` if `target` is a URI (i.e. it begins with a scheme followed
/// by `://`, such as `https://`, `http://`, or `ftp://`).
fn is_uri(target: &str) -> bool {
    URI_PREFIX.is_match(target)
}

/// Returns `true` if `value` names the UTF-8 encoding. The comparison is
/// case-insensitive and ignores a hyphen, so `utf-8`, `UTF-8`, `utf8`, and
/// `UTF8` are all recognized.
fn is_utf8_encoding(value: &str) -> bool {
    let normalized: String = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|&c| c != '-')
        .collect();
    normalized == "utf8"
}

/// Split a delimited attribute value (`lines`/`tags`) into its entries. Per the
/// spec, a comma is used as the separator if one is present; otherwise a
/// semicolon is used (which is why a comma-separated list must be quoted while
/// a semicolon-separated list need not be). Empty entries are dropped.
fn split_delimited_value(value: &str) -> impl Iterator<Item = &str> {
    let delimiter = if value.contains(',') { ',' } else { ';' };
    value
        .split(delimiter)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Apply the `lines` or `tag(s)` selection of an include directive to the raw
/// included text, returning the selected lines (each terminated by a line
/// feed).
///
/// If neither attribute is present the text is returned unchanged. The `lines`
/// attribute takes precedence over `tag(s)` when both are given, matching
/// Asciidoctor.
///
/// The second element of the returned tuple carries any
/// [`TagFilterDiagnostic`]s raised while resolving a `tag(s)` selection (a
/// requested tag that was not found, or a malformed tag directive within the
/// include file); the caller turns each into a warning located at the include
/// directive.
///
/// See `include-lines.adoc` and `include-tagged-regions.adoc`.
fn select_included_lines(
    text: &str,
    attrlist: &Attrlist<'_>,
) -> (String, Vec<TagFilterDiagnostic>) {
    if let Some(lines) = attrlist
        .named_attribute("lines")
        .map(|a| a.value())
        .filter(|v| !v.is_empty())
    {
        return (select_by_line_ranges(text, lines), vec![]);
    }

    // `tag` (singular) and `tags` (plural) are equivalent; the singular form is
    // conventionally used for a single tag but accepts the same syntax.
    if let Some(tags) = attrlist
        .named_attribute("tags")
        .or_else(|| attrlist.named_attribute("tag"))
        .map(|a| a.value())
        .filter(|v| !v.is_empty())
    {
        return select_by_tags(text, tags);
    }

    (text.to_string(), vec![])
}

/// A problem detected while applying a `tag(s)` include selection, to be
/// reported (by the caller) as a warning located at the include directive.
#[derive(Debug)]
enum TagFilterDiagnostic {
    /// One or more requested (non-negated) tags were never found in the include
    /// file. Carries the missing tag names in the order they were requested.
    NotFound(Vec<String>),

    /// A tagged region was opened but never closed before the end of the file.
    Unclosed(String),

    /// An `end::` directive named a tag other than the one currently open. The
    /// fields are the expected (open) tag and the tag actually found.
    MismatchedEnd { expected: String, found: String },

    /// An `end::` directive was found with no corresponding open region.
    UnexpectedEnd(String),
}

/// Select the lines of `text` that fall within any of the ranges named in the
/// `lines` attribute value (`spec`). A single line number is a range of one
/// line; `from..to` is inclusive; an empty or negative end (`from..` or
/// `from..-1`) extends to the end of the file.
fn select_by_line_ranges(text: &str, spec: &str) -> String {
    // Each range is `(from, Some(to))` or `(from, None)` for an open-ended range.
    let ranges: Vec<(usize, Option<usize>)> = split_delimited_value(spec)
        .map(|entry| {
            if let Some((from, to)) = entry.split_once("..") {
                // A non-numeric start coerces to 0, matching Ruby `String#to_i`.
                // Since line numbers are 1-based this behaves the same as a start
                // of 1 (the range still begins at the first line).
                let from = from.trim().parse().unwrap_or(0);
                let to = to.trim();
                let to = match to.parse::<i64>() {
                    Ok(to) if to >= 0 => Some(to as usize),
                    // Empty, `-1`, or any negative value extends to the last line.
                    _ => None,
                };
                (from, to)
            } else {
                let n = entry.parse().unwrap_or(0);
                (n, Some(n))
            }
        })
        // A reversed range (`from` past a concrete `to`, e.g. `10..5`) selects no
        // lines, so it is dropped. If every range is invalid this way, the
        // `lines` attribute is ignored entirely (see below), matching
        // Asciidoctor.
        .filter(|&(from, to)| to.is_none_or(|to| from <= to))
        .collect();

    // With no valid range remaining, the `lines` attribute is ignored and the
    // whole file is included (rather than nothing).
    if ranges.is_empty() {
        return text.to_string();
    }

    let mut output = String::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if ranges
            .iter()
            .any(|&(from, to)| line_number >= from && to.is_none_or(|to| line_number <= to))
        {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

/// Select the lines of `text` enclosed by the tagged regions named in the
/// `tag(s)` attribute value (`spec`), following the tag-filtering rules in
/// `include-tagged-regions.adoc`. Lines that contain a tag directive are always
/// discarded.
fn select_by_tags(text: &str, spec: &str) -> (String, Vec<TagFilterDiagnostic>) {
    let mut diagnostics: Vec<TagFilterDiagnostic> = vec![];

    // Build the ordered set of tag directives, mapping each name to whether it
    // is included (`true`) or excluded (`!name` -> `false`).
    let mut inc_tags: Vec<(String, bool)> = vec![];
    for entry in split_delimited_value(spec) {
        let (name, include) = match entry.strip_prefix('!') {
            Some(name) => (name, false),
            None => (entry, true),
        };
        // Skip an empty entry or a lone `!` (which has no tag name).
        if name.is_empty() {
            continue;
        }
        match inc_tags.iter_mut().find(|(n, _)| n == name) {
            Some(existing) => existing.1 = include,
            None => inc_tags.push((name.to_string(), include)),
        }
    }

    // The set of requested, non-negated tag names (excluding the `*`/`**`
    // wildcards), in request order — used to report any that are never found.
    let requested_named: Vec<String> = inc_tags
        .iter()
        .filter(|(name, include)| *include && name != "*" && name != "**")
        .map(|(name, _)| name.clone())
        .collect();

    // Every tag name opened by a `tag::` directive in the file, so a requested
    // tag that does appear is not reported as missing.
    let mut seen_tags: Vec<String> = vec![];

    // Resolve the base selection (whether lines outside any tag are kept) and
    // the wildcard (the default selection for an unnamed tagged region), then
    // remove the wildcard entries from the named set. This mirrors Asciidoctor.
    let take = |tags: &mut Vec<(String, bool)>, name: &str| -> Option<bool> {
        tags.iter()
            .position(|(n, _)| n == name)
            .map(|i| tags.remove(i).1)
    };

    let mut wildcard: Option<bool> = None;
    let base_select: bool;

    if let Some(double) = take(&mut inc_tags, "**") {
        base_select = double;
        if let Some(single) = take(&mut inc_tags, "*") {
            wildcard = Some(single);
        } else if !double && inc_tags.first().map(|(_, v)| *v) == Some(false) {
            wildcard = Some(true);
        }
    } else if inc_tags.iter().any(|(n, _)| n == "*") {
        if inc_tags.first().map(|(n, _)| n.as_str()) == Some("*") {
            let single = take(&mut inc_tags, "*").unwrap_or(false);
            wildcard = Some(single);
            base_select = !single;
        } else {
            wildcard = take(&mut inc_tags, "*");
            base_select = false;
        }
    } else {
        // With only named inclusions/exclusions, non-tagged lines are kept only
        // when every named tag is an exclusion.
        base_select = !inc_tags.iter().any(|(_, v)| *v);
    }

    let lookup = |name: &str| inc_tags.iter().find(|(n, _)| n == name).map(|(_, v)| *v);

    let mut output = String::new();
    let mut select = base_select;
    let mut active_tag: Option<String> = None;
    // Each entry records the tag name and the `select` state to restore when the
    // region is closed.
    let mut tag_stack: Vec<(String, bool)> = vec![];

    for line in text.lines() {
        if let Some((is_end, name)) = find_tag_directive(line) {
            if is_end {
                if active_tag.as_deref() == Some(name) {
                    tag_stack.pop();
                    match tag_stack.last() {
                        Some((tag, sel)) => {
                            active_tag = Some(tag.clone());
                            select = *sel;
                        }
                        None => {
                            active_tag = None;
                            select = base_select;
                        }
                    }
                } else if let Some(idx) = tag_stack.iter().rposition(|(n, _)| n == name) {
                    // The named region is open, but it is not the innermost one:
                    // an inner region was left unclosed. Report the mismatch and
                    // close the named region (the still-open inner regions are
                    // reported as unclosed at end of file). This matches
                    // Asciidoctor, which removes the matched entry from the stack
                    // while leaving the active (innermost) region in effect.
                    diagnostics.push(TagFilterDiagnostic::MismatchedEnd {
                        expected: active_tag.clone().unwrap_or_default(),
                        found: name.to_string(),
                    });
                    tag_stack.remove(idx);
                } else {
                    // No open region for this tag at all.
                    diagnostics.push(TagFilterDiagnostic::UnexpectedEnd(name.to_string()));
                }
            } else {
                if !seen_tags.iter().any(|n| n == name) {
                    seen_tags.push(name.to_string());
                }
                // Every tagged region is pushed onto the stack so its `end::`
                // directive matches (and an unclosed region is detected),
                // regardless of whether it is selected. Only the `select` state
                // it carries depends on the request.
                select = if let Some(named) = lookup(name) {
                    named
                } else if let Some(wildcard) = wildcard {
                    // An unnamed region uses the wildcard default, unless we are
                    // already inside an unselected region (then it stays excluded).
                    if active_tag.is_some() && !select {
                        false
                    } else {
                        wildcard
                    }
                } else {
                    // A region that is neither requested nor covered by a
                    // wildcard is tracked but leaves the current selection
                    // unchanged (it inherits the enclosing region's state).
                    select
                };
                tag_stack.push((name.to_string(), select));
                active_tag = Some(name.to_string());
            }
            // Directive lines are never emitted.
        } else if select {
            output.push_str(line);
            output.push('\n');
        }
    }

    // Any region still open at end of file was never closed.
    for (name, _) in &tag_stack {
        diagnostics.push(TagFilterDiagnostic::Unclosed(name.clone()));
    }

    // Any requested (non-negated) tag that never appeared as a directive is
    // reported together, in the order the tags were requested.
    let missing: Vec<String> = requested_named
        .into_iter()
        .filter(|name| !seen_tags.iter().any(|n| n == name))
        .collect();
    if !missing.is_empty() {
        diagnostics.push(TagFilterDiagnostic::NotFound(missing));
    }

    (output, diagnostics)
}

/// Locate a tag directive (`tag::NAME[]` or `end::NAME[]`) within `line`.
///
/// Returns `(is_end, name)` when found. The directive must follow a word
/// boundary and be followed by a space, a carriage return, or the end of the
/// line, matching Asciidoctor's `TagDirectiveRx`.
fn find_tag_directive(line: &str) -> Option<(bool, &str)> {
    if !line.contains("::") || !line.contains("[]") {
        return None;
    }

    for caps in TAG_DIRECTIVE.captures_iter(line) {
        // The `regex` crate has no look-ahead, so verify the trailing context
        // (end of line, space, or carriage return) manually.
        let whole = caps.get(0)?;
        let trailing_ok = match line[whole.end()..].chars().next() {
            None => true,
            Some(c) => c == ' ' || c == '\r',
        };
        if trailing_ok {
            let is_end = &caps[1] == "end";
            return Some((is_end, caps.get(2)?.as_str()));
        }
    }

    None
}

/// Normalize the block indentation of included content per the `indent`
/// attribute, returning the adjusted text. If `indent` is absent (or negative)
/// the text is returned unchanged. See `include-with-indent.adoc`.
fn reindent_included_lines(text: String, attrlist: &Attrlist<'_>, parser: &Parser) -> String {
    let Some(indent) = attrlist.named_attribute("indent").map(|a| a.value()) else {
        return text;
    };

    // Asciidoctor coerces the value with `String#to_i` (a non-numeric value
    // yields 0). A negative value disables normalization.
    let indent: i64 = indent.trim().parse().unwrap_or(0);
    if indent < 0 {
        return text;
    }

    let tab_size = match parser.attribute_value("tabsize") {
        InterpretedValue::Value(v) => v.trim().parse().unwrap_or(0),
        _ => 0,
    };

    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    adjust_indentation(&mut lines, indent as usize, tab_size);

    let mut output = lines.join("\n");
    if !output.is_empty() || !text.is_empty() {
        output.push('\n');
    }
    output
}

/// Strip the common leading block indent from `lines` and, when `indent` is
/// greater than zero, re-indent each non-empty line by that many spaces. When
/// `tab_size` is greater than zero, leading tabs are first expanded to spaces.
///
/// Per the spec, if any line in the content is not indented (the common indent
/// is zero) the `indent` normalization is skipped entirely.
fn adjust_indentation(lines: &mut [String], indent: usize, tab_size: usize) {
    if lines.is_empty() {
        return;
    }

    if tab_size > 0 && lines.iter().any(|l| l.contains('\t')) {
        for line in lines.iter_mut() {
            *line = expand_tabs(line, tab_size);
        }
    }

    // The common indent is the minimum count of leading spaces across the
    // non-empty lines.
    let Some(offset) = lines
        .iter()
        .filter(|l| !l.is_empty())
        .map(|l| l.len() - l.trim_start_matches(' ').len())
        .min()
    else {
        return;
    };

    if offset == 0 {
        // At least one line is flush left, so the `indent` attribute is ignored.
        return;
    }

    let padding = " ".repeat(indent);
    for line in lines.iter_mut() {
        if line.is_empty() {
            continue;
        }
        // Leading spaces are ASCII, so slicing by byte offset is safe.
        let stripped = &line[offset..];
        *line = if indent > 0 {
            format!("{padding}{stripped}")
        } else {
            stripped.to_string()
        };
    }
}

/// Expand tabs in `line` to spaces, advancing to the next multiple of
/// `tab_size` (a proper tab stop, not a fixed number of spaces).
fn expand_tabs(line: &str, tab_size: usize) -> String {
    if !line.contains('\t') {
        return line.to_string();
    }

    let mut output = String::new();
    let mut column = 0;
    for ch in line.chars() {
        if ch == '\t' {
            let spaces = tab_size - (column % tab_size);
            output.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        } else {
            output.push(ch);
            column += 1;
        }
    }
    output
}

static TAG_DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    // Matches `tag::NAME[]` / `end::NAME[]` following a word boundary. The
    // trailing context (end of line, space, or carriage return) is checked
    // separately in `find_tag_directive` because the `regex` crate lacks
    // look-ahead. Group 1 captures the keyword; group 2 the (non-space) name.
    Regex::new(r#"\b(tag|end)::(\S+?)\[\]"#).unwrap()
});

static URI_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    // A scheme (letter followed by letters/digits/`.`/`+`/`-`) plus `://`.
    Regex::new(r#"^[A-Za-z][A-Za-z0-9.+-]*://"#).unwrap()
});

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

static CONDITIONAL_DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)                          # Extended (verbose) mode

        ^                               # Start of line

        (\\)?                           # (1) Optional escaping backslash

        (ifdef|ifndef|ifeval|endif)     # (2) Directive keyword

        ::                              # Literal '::' separator

        ([^\[]*)                        # (3) Target (attribute expression), may be empty

        \[                             # Literal '[' opening the brackets

        (.*)                            # (4) Bracketed content, may be empty

        \]                             # Literal closing ']'

        $                               # End of line
        "#,
    )
    .unwrap()
});

static IFEVAL_EXPRESSION: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"(?s)^(.+?)\s*(==|!=|<=|>=|<|>)\s*(.+)$"#).unwrap()
});

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::unwrap_used)]

    use crate::{
        SafeMode,
        attributes::Attrlist,
        parser::{IncludeContent, IncludeFileHandler, SourceLine, preprocessor::preprocess},
        tests::{fixtures::inline_file_handler::InlineFileHandler, prelude::*},
    };

    #[test]
    fn no_preprocessor_directives() {
        let source =
            "= Document Title\n\nThis is a simple document with no includes or conditionals.";
        let parser = Parser::default().with_primary_file_name("test.adoc");

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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
    fn include_directive_at_start_secure_mode() {
        // In secure mode (the default) the include directive on the very first
        // line of a named primary file is converted to a link. Because no
        // earlier line has been emitted yet, this is where the source map is
        // first anchored to the including file, so output line 1 must map back
        // to `main.adoc` line 1 (not an anonymous `None` file).
        let source = "include::header.adoc[]\n\n= Document Title\n\nContent here.";

        let handler = InlineFileHandler::from_pairs([("header.adoc", "SHOULD NOT APPEAR")]);

        let parser = Parser::default()
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

        // The include is converted to a link; the handler is never consulted.
        assert_eq!(
            processed_source,
            "link:header.adoc[role=include]\n\n= Document Title\n\nContent here.\n"
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
    fn include_directive_after_content_secure_mode() {
        // Companion to `include_directive_at_start_secure_mode`: here the
        // include directive is preceded by ordinary content, so the source map
        // has already been anchored to the including file by the time the
        // directive is reached. The secure-mode branch must therefore *not*
        // re-anchor it; the include is still converted to a link and the 1:1
        // mapping back to `main.adoc` is preserved across the directive.
        let source = "= Document Title\n\nSome content.\n\ninclude::header.adoc[]\n\nMore content.";

        let handler = InlineFileHandler::from_pairs([("header.adoc", "SHOULD NOT APPEAR")]);

        let parser = Parser::default()
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

        // The include is converted to a link; the handler is never consulted.
        assert_eq!(
            processed_source,
            "= Document Title\n\nSome content.\n\nlink:header.adoc[role=include]\n\nMore content.\n"
        );

        // The include-turned-link maps back to its own line in `main.adoc`, and
        // the surrounding content keeps its 1:1 mapping.
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("main.adoc".to_owned()), 4))
        );
        assert_eq!(
            source_map.original_file_and_line(5),
            Some(SourceLine(Some("main.adoc".to_owned()), 5))
        );
        assert_eq!(
            source_map.original_file_and_line(6),
            Some(SourceLine(Some("main.adoc".to_owned()), 6))
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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, source_map, warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, source_map, warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, _source_map, _warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, source_map, warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, _source_map, _warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, source_map, warnings, _includes) = preprocess(source, &parser);

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

    /// A parser that resolves `partial.adoc` (and nothing else) with
    /// `attribute-missing` set to `mode`.
    fn parser_with_attribute_missing(mode: &str) -> Parser {
        Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_intrinsic_attribute("attribute-missing", mode, ModificationContext::Anywhere)
            .with_include_file_handler(InlineFileHandler::from_pairs([(
                "partial.adoc",
                "Included content.",
            )]))
    }

    #[test]
    fn include_target_with_missing_attribute_is_skipped_by_default() {
        // The default `attribute-missing=skip` mode keeps the reference literal
        // and still consults the include file handler, which reports no such
        // file.
        let source = "Before.\n\ninclude::{foodir}/partial.adoc[]\n\nAfter.";

        let (processed_source, _source_map, warnings, _includes) =
            preprocess(source, &parser_with_attribute_missing("skip"));

        assert_eq!(
            processed_source,
            "Before.\n\nUnresolved directive in main.adoc - include::{foodir}/partial.adoc[]\n\nAfter.\n"
        );

        assert_eq!(warnings.len(), 1);

        assert_eq!(
            warnings[0].warning,
            WarningType::IncludeFileNotFound("{foodir}/partial.adoc".to_owned())
        );
    }

    #[test]
    fn include_target_with_missing_attribute_is_dropped_under_drop_line() {
        // `attribute-missing=drop-line` drops the entire directive line: nothing
        // is emitted in its place, no warning is recorded, and the include file
        // handler is never consulted. The source map stays aligned for the lines
        // that follow. See issue #776.
        let source = "Before.\n\ninclude::{foodir}/partial.adoc[]\n\nAfter.";

        let (processed_source, source_map, warnings, _includes) =
            preprocess(source, &parser_with_attribute_missing("drop-line"));

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
    fn include_target_with_missing_attribute_is_dropped_at_secure_safe_mode() {
        // The `attribute-missing` policy is applied before the safe-mode link
        // conversion, so a dropped directive does not become a link either
        // (matching Asciidoctor).
        let source = "Before.\n\ninclude::{foodir}/partial.adoc[]\n\nAfter.";

        let parser = Parser::default()
            .with_primary_file_name("main.adoc")
            .with_intrinsic_attribute(
                "attribute-missing",
                "drop-line",
                ModificationContext::Anywhere,
            );

        let (processed_source, _source_map, warnings, _includes) = preprocess(source, &parser);

        assert_eq!(processed_source, "Before.\n\n\nAfter.\n");
        assert!(warnings.is_empty());
    }

    #[test]
    fn include_target_with_missing_attribute_warns_under_warn() {
        // `attribute-missing=warn` leaves the "Unresolved directive" message in
        // place of the directive and records a warning naming the whole
        // directive (Asciidoctor maps `warn` to `drop-line` when substituting an
        // include target, so the target is emptied and never resolved).
        let source = "Before.\n\ninclude::{foodir}/partial.adoc[]\n\nAfter.";

        let (processed_source, _source_map, warnings, _includes) =
            preprocess(source, &parser_with_attribute_missing("warn"));

        assert_eq!(
            processed_source,
            "Before.\n\nUnresolved directive in main.adoc - include::{foodir}/partial.adoc[]\n\nAfter.\n"
        );

        assert_eq!(warnings.len(), 1);

        assert_eq!(
            warnings[0].warning,
            WarningType::IncludeDroppedDueToMissingAttribute(
                "include::{foodir}/partial.adoc[]".to_owned()
            )
        );
    }

    #[test]
    fn optional_include_target_with_missing_attribute_is_dropped_silently_under_warn() {
        // `opts=optional` suppresses the "Unresolved directive" text and the
        // warning that `warn` would otherwise produce for a dropped directive.
        let source = "Before.\n\ninclude::{foodir}/partial.adoc[opts=optional]\n\nAfter.";

        let (processed_source, _source_map, warnings, _includes) =
            preprocess(source, &parser_with_attribute_missing("warn"));

        assert_eq!(processed_source, "Before.\n\n\nAfter.\n");
        assert!(warnings.is_empty());
    }

    #[test]
    fn include_target_with_missing_attribute_is_still_resolved_under_drop() {
        // `attribute-missing=drop` removes only the reference, so a target that
        // is otherwise complete still resolves and the include is expanded.
        let source = "Before.\n\ninclude::{foodir}partial.adoc[]\n\nAfter.";

        let (processed_source, _source_map, warnings, _includes) =
            preprocess(source, &parser_with_attribute_missing("drop"));

        assert_eq!(processed_source, "Before.\n\nIncluded content.\n\nAfter.\n");

        assert!(warnings.is_empty());
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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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
        let (processed_source, _source_map, _warnings, _includes) = preprocess(source, &parser);
        assert_eq!(processed_source, "include::partial.adoc[]\n");
    }

    #[test]
    fn escaped_non_directive_is_unchanged() {
        // A backslash followed by something that is not a valid include directive
        // (here, no attribute brackets) is left untouched.
        let source = "\\include::partial.adoc";
        let parser = Parser::default().with_primary_file_name("main.adoc");
        let (processed_source, _source_map, _warnings, _includes) = preprocess(source, &parser);
        assert_eq!(processed_source, "\\include::partial.adoc\n");
    }

    #[test]
    fn double_backslash_include_is_unchanged() {
        // Only a single leading backslash is treated as an escape; a double
        // backslash is left as-is.
        let source = "\\\\include::partial.adoc[]";
        let parser = Parser::default().with_primary_file_name("main.adoc");
        let (processed_source, _source_map, _warnings, _includes) = preprocess(source, &parser);
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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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
    fn dropped_include_attrlist_does_not_leak_counter_state() {
        // Checking `opts=optional` on a directive that is about to be dropped
        // means parsing its attribute list, which applies substitutions — so a
        // stateful expression such as `{counter:n}` is evaluated there. It
        // cannot be observed afterward: the preprocessor runs against a
        // throwaway clone of the parser, so every attribute and counter it
        // touches dies with that clone. The counter in the surviving paragraph
        // is therefore the first value of the sequence.
        let source = ":attribute-missing: warn\n\ninclude::{foodir}/partial.adoc[opts=optional,title={counter:n}]\n\nValue: {counter:n}.";

        let mut parser = Parser::default();
        let doc = parser.parse(source);

        assert_eq!(parser.attribute_value("n"), InterpretedValue::Value("1"));

        let rendered: Vec<_> = doc
            .nested_blocks()
            .filter_map(|b| b.rendered_content())
            .collect();

        assert_eq!(rendered, vec!["Value: 1."]);
    }

    #[test]
    fn include_target_with_brace_that_is_not_an_attribute_reference() {
        // A `{` that doesn't open a well-formed attribute reference gets past
        // the fast path but matches nothing, so the target is used verbatim —
        // and it is not treated as a missing reference under any
        // `attribute-missing` policy.
        let source = "include::{}partial.adoc[]";

        let handler =
            InlineFileHandler::from_pairs([("{}partial.adoc", "Brace in the file name.")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_intrinsic_attribute(
                "attribute-missing",
                "drop-line",
                ModificationContext::Anywhere,
            )
            .with_include_file_handler(handler);

        let (processed_source, _source_map, warnings, _includes) = preprocess(source, &parser);

        assert_eq!(processed_source, "Brace in the file name.\n");
        assert!(warnings.is_empty());
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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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
    fn escaped_attribute_reference_in_include_target_drops_backslash() {
        // An escaped reference (`\{missing}`) has its backslash removed during
        // preprocessing even when the attribute is unset, so the include target
        // resolves against the literal `{missing}` form rather than retaining
        // the backslash. This matches the content-substitution path and
        // Asciidoctor (see issue #667).
        let source = "include::pre\\{missing}post.adoc[]";

        let handler = InlineFileHandler::from_pairs([(
            "pre{missing}post.adoc",
            "Included via escaped literal target.",
        )]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, _source_map, _warnings, _includes) = preprocess(source, &parser);

        assert_eq!(processed_source, "Included via escaped literal target.\n");
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

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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
    fn attribute_substitution_in_target_with_attrlist() {
        // The target is resolved via attribute substitution and a `tag`
        // attribute selects only the tagged region (the tag directive lines
        // themselves are discarded).
        let source = ":srcdir: examples\n:lang: java\n\ninclude::{srcdir}/hello.{lang}[tag=main]";

        let handler = InlineFileHandler::from_pairs([(
            "examples/hello.java",
            "// tag::main[]\npublic class Hello {}\n// end::main[]",
        )]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

        assert_eq!(
            processed_source,
            ":srcdir: examples\n:lang: java\n\npublic class Hello {}\n"
        );

        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(Some("examples/hello.java".to_owned()), 1))
        );
    }

    #[test]
    fn attribute_substitution_with_multiline_attribute() {
        let source = ":longpath: very/long/path/to/some/ \\\nsubdirectory\n:ext: adoc\n\ninclude::{longpath}/file.{ext}[]";

        // A soft-wrap line continuation folds the ` \` marker, the newline, and any
        // ensuing indentation into a single space (see the `wrap_values` spec test).
        // So `{longpath}` correctly resolves to "very/long/path/to/some/ subdirectory"
        // *with* the space, matching Asciidoctor. The space is inherent to soft
        // wrapping, not a stray artifact.
        let handler = InlineFileHandler::from_pairs([(
            "very/long/path/to/some/ subdirectory/file.adoc",
            "Multi-line attribute worked!",
        )]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (processed_source, source_map, _warnings, _includes) = preprocess(source, &parser);

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

    /// Preprocess `source` with a default (secure) parser and return only the
    /// resulting text, for the conditional-directive tests below.
    fn conditional_output(source: &str) -> String {
        let parser = Parser::default();
        let (output, _source_map, _warnings, _includes) = preprocess(source, &parser);
        output
    }

    #[test]
    fn ifdef_set_includes_content() {
        assert_eq!(
            conditional_output(":foo:\n\nifdef::foo[]\nkept\nendif::[]\n\ntail"),
            ":foo:\n\nkept\n\ntail\n"
        );
    }

    #[test]
    fn ifdef_unset_excludes_content() {
        assert_eq!(
            conditional_output("head\n\nifdef::foo[]\ndropped\nendif::[]\n\ntail"),
            "head\n\n\ntail\n"
        );
    }

    #[test]
    fn ifndef_unset_includes_content() {
        assert_eq!(
            conditional_output("head\n\nifndef::foo[]\nkept\nendif::[]"),
            "head\n\nkept\n"
        );
    }

    #[test]
    fn ifndef_set_excludes_content() {
        assert_eq!(
            conditional_output(":foo:\n\nifndef::foo[]\ndropped\nendif::[]\n\ntail"),
            ":foo:\n\n\ntail\n"
        );
    }

    #[test]
    fn ifdef_single_line_included() {
        assert_eq!(
            conditional_output(":foo:\n\nifdef::foo[kept on one line]"),
            ":foo:\n\nkept on one line\n"
        );
    }

    #[test]
    fn ifdef_single_line_excluded() {
        assert_eq!(
            conditional_output("head\n\nifdef::foo[dropped]\n\ntail"),
            "head\n\n\ntail\n"
        );
    }

    #[test]
    fn comment_block_suppresses_conditional_directive() {
        // A conditional directive inside a `////` comment block is not
        // processed: the block's content is emitted verbatim so it parses as a
        // comment (see issue #810). `foo` is unset, so were the directive
        // processed, `hidden` would be dropped and the block corrupted.
        assert_eq!(
            conditional_output("////\nifdef::foo[]\nhidden\nendif::[]\n////\n\ntail"),
            "////\nifdef::foo[]\nhidden\nendif::[]\n////\n\ntail\n"
        );
    }

    #[test]
    fn longer_comment_delimiter_closes_only_on_exact_match() {
        // A comment block closes on a line matching its opening delimiter
        // exactly; a shorter run of slashes inside it is ordinary content.
        assert_eq!(
            conditional_output("/////\nifdef::foo[x]\n////\nstill in comment\n/////\ntail"),
            "/////\nifdef::foo[x]\n////\nstill in comment\n/////\ntail\n"
        );
    }

    #[test]
    fn comment_block_suppresses_include_expansion() {
        // An include directive inside a comment block is likewise left
        // untouched rather than expanded.
        let source = "////\ninclude::sub.adoc[]\n////\n\ntail";
        let handler = InlineFileHandler::from_pairs([("sub.adoc", "Included.")]);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (output, _source_map, _warnings, _includes) = preprocess(source, &parser);
        assert_eq!(output, "////\ninclude::sub.adoc[]\n////\n\ntail\n");
    }

    #[test]
    fn comment_open_block_suppresses_conditional_directive() {
        // A `[comment]`-styled open block (`--`) is a comment block: a directive
        // within it is emitted verbatim.
        assert_eq!(
            conditional_output("[comment]\n--\nfirst\nifdef::foo[dropped]\nlast\n--\n\ntail"),
            "[comment]\n--\nfirst\nifdef::foo[dropped]\nlast\n--\n\ntail\n"
        );
    }

    #[test]
    fn comment_paragraph_suppresses_directive_after_first_line() {
        // In a `[comment]` paragraph the first line is still processed, but its
        // subsequent lines are raw (matching Asciidoctor's one-line
        // look-ahead). The directive on the second line is left untouched.
        assert_eq!(
            conditional_output("[comment]\nfirst line\nifdef::foo[dropped]\n\ntail"),
            "[comment]\nfirst line\nifdef::foo[dropped]\n\ntail\n"
        );
    }

    #[test]
    fn ifdef_single_line_attribute_entry_is_applied() {
        // The attribute set inside the single-line directive is observed by the
        // following directive.
        assert_eq!(
            conditional_output(":foo:\n\nifdef::foo[:bar: yes]\nifdef::bar[bar is set]"),
            ":foo:\n\n:bar: yes\nbar is set\n"
        );
    }

    #[test]
    fn single_line_attribute_entry_preserves_attribute_context() {
        // Emitting an attribute-entry line via a single-line conditional must not
        // disable preprocessor attribute handling for the immediately following
        // line (as the main attribute-entry handler leaves it enabled). Here the
        // entry after the directive must still be applied so the include target
        // that references it resolves.
        let source = ":flag:\n\nifdef::flag[:dir: sub]\n:file: {dir}/f\ninclude::{file}.adoc[]";

        let handler = InlineFileHandler::from_pairs([("sub/f.adoc", "Included.")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (output, _source_map, warnings, _includes) = preprocess(source, &parser);

        assert!(output.contains("Included."), "output was: {output:?}");
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn ifdef_or_combinator() {
        // Comma means "any set": one of the two attributes is enough.
        assert_eq!(
            conditional_output(":b:\n\nifdef::a,b[]\nkept\nendif::[]"),
            ":b:\n\nkept\n"
        );
        assert_eq!(
            conditional_output("head\n\nifdef::a,b[]\ndropped\nendif::[]"),
            "head\n\n"
        );
    }

    #[test]
    fn ifdef_and_combinator() {
        // Plus means "all set": both attributes are required.
        assert_eq!(
            conditional_output(":a:\n:b:\n\nifdef::a+b[]\nkept\nendif::[]"),
            ":a:\n:b:\n\nkept\n"
        );
        assert_eq!(
            conditional_output(":a:\n\nifdef::a+b[]\ndropped\nendif::[]"),
            ":a:\n\n"
        );
    }

    #[test]
    fn nested_conditionals() {
        // The inner directive is only evaluated when the outer one includes.
        assert_eq!(
            conditional_output(
                ":outer:\n:inner:\n\nifdef::outer[]\nA\nifdef::inner[]\nB\nendif::[]\nC\nendif::[]"
            ),
            ":outer:\n:inner:\n\nA\nB\nC\n"
        );
    }

    #[test]
    fn nested_conditional_inside_skipped_region_stays_skipped() {
        // When the outer condition is false the whole region is dropped even
        // though the inner condition would be true on its own.
        assert_eq!(
            conditional_output(
                ":inner:\n\nifdef::outer[]\nA\nifdef::inner[]\nB\nendif::[]\nC\nendif::[]\n\ntail"
            ),
            ":inner:\n\n\ntail\n"
        );
    }

    #[test]
    fn named_endif_matches_target() {
        assert_eq!(
            conditional_output(":foo:\n\nifdef::foo[]\nkept\nendif::foo[]"),
            ":foo:\n\nkept\n"
        );
    }

    #[test]
    fn ifeval_numeric_true() {
        assert_eq!(
            conditional_output("head\n\nifeval::[2 > 1]\nkept\nendif::[]"),
            "head\n\nkept\n"
        );
    }

    #[test]
    fn ifeval_numeric_false() {
        assert_eq!(
            conditional_output("head\n\nifeval::[1 > 2]\ndropped\nendif::[]\n\ntail"),
            "head\n\n\ntail\n"
        );
    }

    #[test]
    fn ifeval_attribute_reference() {
        // `sectnumlevels` defaults to 3.
        assert_eq!(
            conditional_output("head\n\nifeval::[{sectnumlevels} == 3]\nkept\nendif::[]"),
            "head\n\nkept\n"
        );
    }

    #[test]
    fn ifeval_string_comparison() {
        assert_eq!(
            conditional_output(
                ":backend: html5\n\nifeval::[\"{backend}\" == \"html5\"]\nkept\nendif::[]"
            ),
            ":backend: html5\n\nkept\n"
        );
        assert_eq!(
            conditional_output(
                ":backend: docbook5\n\nifeval::[\"{backend}\" == \"html5\"]\ndropped\nendif::[]"
            ),
            ":backend: docbook5\n\n"
        );
    }

    #[test]
    fn ifeval_type_mismatch_is_false() {
        // Comparing a number and a string with an ordering operator fails, so
        // the content is skipped.
        assert_eq!(
            conditional_output("head\n\nifeval::[1 < \"a\"]\ndropped\nendif::[]\n\ntail"),
            "head\n\n\ntail\n"
        );
    }

    #[test]
    fn escaped_conditional_directive_emitted_literally() {
        assert_eq!(
            conditional_output("head\n\n\\ifdef::foo[]\n\ntail"),
            "head\n\nifdef::foo[]\n\ntail\n"
        );
    }

    #[test]
    fn source_map_realigns_after_skipped_region() {
        // Lines dropped by a false conditional must not corrupt the mapping of
        // the lines that follow back to their original line numbers.
        let source = "l1\n\nifdef::foo[]\ndropped\ndropped\nendif::[]\n\nl8";
        let parser = Parser::default();
        let (output, source_map, _warnings, _includes) = preprocess(source, &parser);

        assert_eq!(output, "l1\n\n\nl8\n");

        // Output line 1 -> source line 1.
        assert_eq!(
            source_map.original_file_and_line(1),
            Some(SourceLine(None, 1))
        );
        // Output line 4 ("l8") -> source line 8.
        assert_eq!(
            source_map.original_file_and_line(4),
            Some(SourceLine(None, 8))
        );
    }

    #[test]
    fn ifeval_with_nonempty_target_is_malformed() {
        // `ifeval` requires an empty target; a non-empty one is malformed and
        // opens no conditional, so the following lines are emitted unchanged.
        assert_eq!(
            conditional_output("ifeval::foo[1 == 1]\nkept\nendif::[]"),
            "kept\n"
        );
    }

    #[test]
    fn ifdef_with_empty_target_is_malformed() {
        // `ifdef`/`ifndef` require a target; an empty one is malformed and opens
        // no conditional.
        assert_eq!(conditional_output("ifdef::[]\nkept\nendif::[]"), "kept\n");
    }

    #[test]
    fn ifeval_malformed_expression_is_dropped() {
        // An expression with no comparison operator cannot be parsed, so the
        // directive is malformed: it opens no conditional and the following
        // lines are emitted unchanged (the stray `endif::[]` is then unmatched
        // and simply discarded), matching Asciidoctor.
        assert_eq!(
            conditional_output("ifeval::[nonsense]\nkept\nendif::[]\n\ntail"),
            "kept\n\ntail\n"
        );
    }

    #[test]
    fn ifeval_coerces_trailing_text_to_integer() {
        // An unquoted value with no period coerces to its leading integer
        // (Ruby `String#to_i`), so `3x` becomes `3`.
        assert_eq!(
            conditional_output("ifeval::[3x == 3]\nkept\nendif::[]"),
            "kept\n"
        );
    }

    #[test]
    fn ifeval_coerces_trailing_text_to_float() {
        // An unquoted value containing a period coerces to its leading float
        // (Ruby `String#to_f`), so `1.5x` becomes `1.5`.
        assert_eq!(
            conditional_output("ifeval::[1.5x < 2]\nkept\nendif::[]"),
            "kept\n"
        );
    }

    #[test]
    fn ifeval_float_and_mixed_equality() {
        // Float/float and int/float equality.
        assert_eq!(
            conditional_output("ifeval::[1.5 == 1.5]\nkept\nendif::[]"),
            "kept\n"
        );
        assert_eq!(
            conditional_output("ifeval::[2 == 2.0]\nkept\nendif::[]"),
            "kept\n"
        );
        // Equality across incompatible value types is false.
        assert_eq!(
            conditional_output("ifeval::[1 == \"a\"]\ndropped\nendif::[]\n\ntail"),
            "\ntail\n"
        );
    }

    #[test]
    fn ifeval_float_and_string_ordering() {
        // Float/float, int/float, and string/string ordering.
        assert_eq!(
            conditional_output("ifeval::[1.5 < 2.5]\nkept\nendif::[]"),
            "kept\n"
        );
        assert_eq!(
            conditional_output("ifeval::[1 < 2.5]\nkept\nendif::[]"),
            "kept\n"
        );
        assert_eq!(
            conditional_output("ifeval::[\"a\" < \"b\"]\nkept\nendif::[]"),
            "kept\n"
        );
        // `>=` between two comparable values.
        assert_eq!(
            conditional_output("ifeval::[3 >= 3]\nkept\nendif::[]"),
            "kept\n"
        );
    }

    /// Preprocess an `include::sample.adoc[<attrs>]` directive whose target
    /// resolves to `content`, returning the resulting output text.
    fn include_output(attrs: &str, content: &'static str) -> String {
        let source = format!("include::sample.adoc[{attrs}]");
        let handler = InlineFileHandler::from_pairs([("sample.adoc", content)]);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);
        preprocess(&source, &parser).0
    }

    const NUMBERED: &str = "one\ntwo\nthree\nfour\nfive";

    #[test]
    fn lines_single_range() {
        assert_eq!(include_output("lines=2..4", NUMBERED), "two\nthree\nfour\n");
    }

    #[test]
    fn lines_single_line() {
        assert_eq!(include_output("lines=3", NUMBERED), "three\n");
    }

    #[test]
    fn lines_multiple_ranges_semicolon() {
        assert_eq!(
            include_output("lines=1..2;4..5", NUMBERED),
            "one\ntwo\nfour\nfive\n"
        );
    }

    #[test]
    fn lines_multiple_ranges_comma() {
        // A comma-separated list arrives already unquoted from the attrlist.
        assert_eq!(
            include_output("lines=\"1,3,5\"", NUMBERED),
            "one\nthree\nfive\n"
        );
    }

    #[test]
    fn lines_open_ended_range() {
        assert_eq!(
            include_output("lines=3..-1", NUMBERED),
            "three\nfour\nfive\n"
        );
        assert_eq!(include_output("lines=3..", NUMBERED), "three\nfour\nfive\n");
    }

    const TAGGED: &str =
        "// tag::a[]\nalpha\n// tag::b[]\nbeta\n// end::b[]\ngamma\n// end::a[]\ndelta";

    #[test]
    fn tag_selects_region_and_drops_directives() {
        // The nested `b` region is inside `a`, so `tag=a` includes it too.
        assert_eq!(include_output("tag=a", TAGGED), "alpha\nbeta\ngamma\n");
    }

    #[test]
    fn tag_selects_nested_region_only() {
        assert_eq!(include_output("tag=b", TAGGED), "beta\n");
    }

    #[test]
    fn tags_exclude_nested_region() {
        assert_eq!(include_output("tags=a;!b", TAGGED), "alpha\ngamma\n");
    }

    #[test]
    fn tags_double_wildcard_drops_directive_lines() {
        // `**` keeps every line except the tag-directive lines.
        assert_eq!(
            include_output("tags=**", TAGGED),
            "alpha\nbeta\ngamma\ndelta\n"
        );
    }

    #[test]
    fn tags_negated_wildcard_selects_untagged_only() {
        // `!*` keeps only lines outside any tagged region.
        assert_eq!(include_output("tags=!*", TAGGED), "delta\n");
    }

    #[test]
    fn tags_single_wildcard_selects_all_regions() {
        // `*` keeps all tagged regions but not untagged lines.
        assert_eq!(include_output("tags=*", TAGGED), "alpha\nbeta\ngamma\n");
    }

    #[test]
    fn indent_zero_strips_block_indent() {
        let content = "    def names\n      @name.split ' '\n    end";
        assert_eq!(
            include_output("indent=0", content),
            "def names\n  @name.split ' '\nend\n"
        );
    }

    #[test]
    fn indent_positive_reindents_block() {
        let content = "    def names\n      @name.split ' '\n    end";
        assert_eq!(
            include_output("indent=2", content),
            "  def names\n    @name.split ' '\n  end\n"
        );
    }

    #[test]
    fn indent_ignored_when_a_line_is_flush_left() {
        // A line with no indentation makes the common indent zero, so `indent`
        // is effectively ignored.
        let content = "def names\n  @name.split ' '\nend";
        assert_eq!(
            include_output("indent=4", content),
            "def names\n  @name.split ' '\nend\n"
        );
    }

    #[test]
    fn leveloffset_wraps_included_content() {
        // The included content is surrounded by `:leveloffset:` attribute entries
        // that apply and then reset the offset.
        assert_eq!(
            include_output("leveloffset=+1", "== Chapter\n\nBody."),
            ":leveloffset: +1\n\n== Chapter\n\nBody.\n\n:leveloffset!:\n"
        );
    }

    #[test]
    fn uri_include_disabled_without_allow_uri_read() {
        // A URI target is not resolved unless `allow-uri-read` is set; it is
        // reported as an unresolved directive.
        let source = "include::https://example.org/frag.adoc[]";
        let handler =
            InlineFileHandler::from_pairs([("https://example.org/frag.adoc", "SHOULD NOT APPEAR")]);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (output, _source_map, warnings, _includes) = preprocess(source, &parser);

        assert_eq!(
            output,
            "Unresolved directive in main.adoc - include::https://example.org/frag.adoc[]\n"
        );
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn uri_include_resolved_with_allow_uri_read() {
        // With `allow-uri-read` set (and safe mode below secure), the handler is
        // consulted for the URI target.
        let source = "include::https://example.org/frag.adoc[]";
        let handler =
            InlineFileHandler::from_pairs([("https://example.org/frag.adoc", "Remote content.")]);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute("allow-uri-read", "", ModificationContext::Anywhere)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (output, _source_map, warnings, _includes) = preprocess(source, &parser);

        assert_eq!(output, "Remote content.\n");
        assert!(warnings.is_empty());
    }

    #[test]
    fn encoding_utf8_produces_no_warning() {
        // A UTF-8 `encoding` (in any accepted spelling) is honored silently.
        for encoding in ["utf-8", "UTF-8", "utf8", "UTF8"] {
            let output = include_output(&format!("encoding={encoding}"), "Content.");
            assert_eq!(output, "Content.\n");
        }

        let source = "include::sample.adoc[encoding=utf-8]";
        let handler = InlineFileHandler::from_pairs([("sample.adoc", "Content.")]);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);
        let (_output, _source_map, warnings, _includes) = preprocess(source, &parser);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn non_utf8_encoding_warns_but_still_includes() {
        // A non-UTF-8 `encoding` cannot be honored, so a warning is recorded; the
        // content (as provided by the handler) is still merged.
        let source = "include::sample.adoc[encoding=iso-8859-1]";
        let handler = InlineFileHandler::from_pairs([("sample.adoc", "Résumé.")]);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (output, _source_map, warnings, _includes) = preprocess(source, &parser);

        assert_eq!(output, "Résumé.\n");
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::NonUtf8IncludeEncoding("iso-8859-1".to_owned())
        );
        // The warning points at the first line of the included content.
        assert_eq!(
            &output[warnings[0].offset..warnings[0].offset + warnings[0].len],
            "Résumé."
        );
    }

    #[test]
    fn transcoded_include_suppresses_encoding_warning() {
        // A handler that transcodes non-UTF-8 content to UTF-8 itself returns
        // `IncludeContent::transcoded`, which honors the requested `encoding`
        // and suppresses the `NonUtf8IncludeEncoding` warning. See
        // https://github.com/asciidoc-rs/asciidoc-parser/issues/611.
        #[derive(Debug)]
        struct TranscodingFileHandler;

        impl IncludeFileHandler for TranscodingFileHandler {
            fn resolve_target<'src>(
                &self,
                _source: Option<&str>,
                _target: &str,
                _attrlist: &Attrlist<'src>,
                _parser: &Parser,
            ) -> Option<IncludeContent> {
                // Pretend the bytes on disk were `iso-8859-1` and we decoded
                // them; the returned content is valid UTF-8.
                Some(IncludeContent::transcoded("Résumé."))
            }
        }

        let source = "include::sample.adoc[encoding=iso-8859-1]";
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(TranscodingFileHandler);

        let (output, _source_map, warnings, _includes) = preprocess(source, &parser);

        assert_eq!(output, "Résumé.\n");
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn leveloffset_restores_previous_offset() {
        // When a `:leveloffset:` is already in effect, the include restores it to
        // that value (rather than unsetting it) afterward.
        let source = ":leveloffset: 1\n\ninclude::sample.adoc[leveloffset=+1]";
        let handler = InlineFileHandler::from_pairs([("sample.adoc", "== Chapter")]);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (output, _source_map, _warnings, _includes) = preprocess(source, &parser);

        assert_eq!(
            output,
            ":leveloffset: 1\n\n:leveloffset: +1\n\n== Chapter\n\n:leveloffset: 1\n"
        );
    }

    #[test]
    fn leveloffset_restore_ignores_offset_set_within_include() {
        // A `:leveloffset:` set inside the included file must not affect the value
        // restored after the include: the restore reflects the offset in effect
        // *before* the include (here, unset).
        let source = "include::sample.adoc[leveloffset=+1]";
        let handler =
            InlineFileHandler::from_pairs([("sample.adoc", ":leveloffset: 2\n\n== Chapter")]);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (output, _source_map, _warnings, _includes) = preprocess(source, &parser);

        assert_eq!(
            output,
            ":leveloffset: +1\n\n:leveloffset: 2\n\n== Chapter\n\n:leveloffset!:\n"
        );
    }

    #[test]
    fn tag_filtering_edge_cases() {
        // A lone `!` entry (no tag name) is ignored.
        assert_eq!(
            include_output("tags=foo;!", "// tag::foo[]\nx\n// end::foo[]"),
            "x\n"
        );

        // A repeated tag name updates the existing entry (last one wins).
        assert_eq!(
            include_output("tags=!foo;foo", "// tag::foo[]\nx\n// end::foo[]"),
            "x\n"
        );

        // `**` combined with `*` keeps every non-directive line.
        assert_eq!(
            include_output("tags=**;*", "// tag::a[]\nx\n// end::a[]\ny"),
            "x\ny\n"
        );

        // A negated double wildcard combined with an exclusion selects no lines.
        assert_eq!(
            include_output(
                "tags=!**;!foo",
                "before\n// tag::foo[]\nf\n// end::foo[]\nafter"
            ),
            ""
        );

        // A tag directive inside a circumfix comment (followed by a space) is
        // recognized and discarded.
        assert_eq!(
            include_output("tag=x", "<!-- tag::x[] -->\nc\n<!-- end::x[] -->"),
            "c\n"
        );

        // A `tag::` that is not immediately followed by a space or end of line is
        // not a directive, so the line is kept as content.
        assert_eq!(
            include_output("tag=x", "// tag::x[]\ntag::x[]y\n// end::x[]"),
            "tag::x[]y\n"
        );
    }

    #[test]
    fn indent_edge_cases() {
        // A negative `indent` disables normalization; the content is unchanged.
        assert_eq!(
            include_output("indent=-1", "    a\n    b"),
            "    a\n    b\n"
        );

        // Empty content with `indent` is handled without panicking.
        assert_eq!(include_output("indent=0", ""), "");

        // Content that is only blank lines with `indent` is left unchanged.
        assert_eq!(include_output("indent=0", "\n\n"), "\n\n");

        // A blank line interspersed with indented lines is left untouched while
        // the indented lines are re-indented.
        assert_eq!(include_output("indent=2", "    a\n\n    b"), "  a\n\n  b\n");
    }

    #[test]
    fn indent_with_tabsize_and_untabbed_line() {
        // With `tabsize` set, leading tabs are expanded even on a block that also
        // contains a line with no tabs (which is passed through unchanged).
        let source = "----\ninclude::code.rb[indent=0]\n----";
        let handler = InlineFileHandler::from_pairs([("code.rb", "\ta\nno-tab\n\tb")]);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute("tabsize", "4", ModificationContext::Anywhere)
            .with_primary_file_name("main.adoc")
            .with_include_file_handler(handler);

        let (output, _source_map, _warnings, _includes) = preprocess(source, &parser);

        // Tabs expand to the tab stop; the common indent is zero (the middle line
        // is flush left), so no further indentation change is made.
        assert_eq!(output, "----\n    a\nno-tab\n    b\n----\n");
    }

    #[test]
    fn cyclic_include_is_bounded_by_max_include_depth() {
        // A file that includes itself would recurse without limit if the
        // include depth were not enforced. The default `max-include-depth` of
        // 64 bounds the expansion: the directive at the 64th nesting level is
        // left verbatim, with a "maximum include depth exceeded" error.
        let handler = InlineFileHandler::from_pairs([("loop.adoc", "include::loop.adoc[]")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler);

        let (output, _source_map, warnings, _includes) =
            preprocess("include::loop.adoc[]", &parser);

        // Each nesting level's only line is the directive itself, which
        // expands to the next level (contributing no output of its own) until
        // the limit is reached and the directive survives verbatim.
        assert_eq!(output, "include::loop.adoc[]\n");

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::MaxIncludeDepthExceeded(64)
        );
    }

    #[test]
    fn max_include_depth_set_with_no_value_disables_includes() {
        // `max-include-depth` set as a boolean (no value) coerces like an
        // empty string in Ruby (`''.to_i == 0`), so it disables the include
        // directive just as an explicit 0 does: the directive is left
        // verbatim, silently, and the handler is never consulted.
        let handler = InlineFileHandler::from_pairs([("shared.adoc", "shared content")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute_bool("max-include-depth", true, ModificationContext::ApiOnly)
            .with_include_file_handler(handler);

        let (output, _source_map, warnings, _includes) =
            preprocess("include::shared.adoc[]", &parser);

        assert_eq!(output, "include::shared.adoc[]\n");
        assert!(warnings.is_empty());
    }

    #[test]
    fn max_include_depth_unset_falls_back_to_default() {
        // With `max-include-depth` explicitly unset via the API, the
        // preprocessor falls back to Asciidoctor's default of 64, so an
        // ordinary include still expands.
        let handler = InlineFileHandler::from_pairs([("shared.adoc", "shared content")]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute_bool("max-include-depth", false, ModificationContext::ApiOnly)
            .with_include_file_handler(handler);

        let (output, _source_map, warnings, _includes) =
            preprocess("include::shared.adoc[]", &parser);

        assert_eq!(output, "shared content\n");
        assert!(warnings.is_empty());
    }

    #[test]
    fn depth_request_exceeding_max_include_depth_is_clamped() {
        // A `depth` request larger than the absolute `max-include-depth` limit
        // is clamped to it: with a limit of 2, `depth=10` still refuses the
        // third nesting level, and the diagnostic reports the clamped limit
        // (2), not the requested relative depth (10) — matching Asciidoctor.
        let handler = InlineFileHandler::from_pairs([
            ("a.adoc", "include::b.adoc[]"),
            ("b.adoc", "include::c.adoc[]"),
            ("c.adoc", "content of c"),
        ]);

        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute("max-include-depth", "2", ModificationContext::ApiOnly)
            .with_include_file_handler(handler);

        let (output, _source_map, warnings, _includes) =
            preprocess("include::a.adoc[depth=10]", &parser);

        assert_eq!(output, "include::c.adoc[]\n");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].warning, WarningType::MaxIncludeDepthExceeded(2));
    }

    #[test]
    fn huge_max_include_depth_acts_as_large_limit() {
        // A positive `max-include-depth` too large to represent exactly —
        // whether beyond `usize` on a 32-bit target or beyond `i64` entirely —
        // is a very large limit, not the 0 = disabled sentinel: an ordinary
        // include still expands. (Ruby's integers are unbounded, so
        // Asciidoctor honors any such value.)
        for value in ["9223372036854775807", "9223372036854775808"] {
            let handler = InlineFileHandler::from_pairs([("shared.adoc", "shared content")]);

            let parser = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("max-include-depth", value, ModificationContext::ApiOnly)
                .with_include_file_handler(handler);

            let (output, _source_map, warnings, _includes) =
                preprocess("include::shared.adoc[]", &parser);

            assert_eq!(output, "shared content\n");
            assert!(warnings.is_empty());
        }
    }

    #[test]
    fn huge_depth_request_is_clamped_not_wrapped() {
        // A huge positive `depth` request — again, whether beyond `usize` on a
        // 32-bit target or beyond `i64` entirely — is treated like any other
        // greater-than-the-limit request, clamped to the absolute
        // `max-include-depth`, rather than wrapping or collapsing into a small
        // (or zero) value that would restrict nesting further than asked.
        for value in ["9223372036854775807", "9223372036854775808"] {
            let handler = InlineFileHandler::from_pairs([
                ("a.adoc", "include::b.adoc[]"),
                ("b.adoc", "content of b"),
            ]);

            let parser = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("max-include-depth", "1", ModificationContext::ApiOnly)
                .with_include_file_handler(handler);

            let (output, _source_map, warnings, _includes) =
                preprocess(&format!("include::a.adoc[depth={value}]"), &parser);

            assert_eq!(output, "include::b.adoc[]\n");
            assert_eq!(warnings.len(), 1);
            assert_eq!(warnings[0].warning, WarningType::MaxIncludeDepthExceeded(1));
        }
    }

    #[test]
    fn ruby_to_i_saturates_on_overflow() {
        use super::ruby_to_i;

        assert_eq!(ruby_to_i("42"), 42);
        assert_eq!(ruby_to_i("42abc"), 42);

        // Beyond `i64` in either direction saturates by sign (Ruby's unbounded
        // integers keep the value's magnitude; 0 would invert its meaning).
        assert_eq!(ruby_to_i("9223372036854775808"), i64::MAX);
        assert_eq!(ruby_to_i("-9223372036854775809"), i64::MIN);

        // No numeric portion at all still yields 0, as Ruby's `to_i` does.
        assert_eq!(ruby_to_i("abc"), 0);
        assert_eq!(ruby_to_i("-"), 0);
        assert_eq!(ruby_to_i(""), 0);
    }

    mod include_registry {
        use super::super::{include_catalog_key, is_full_include};
        use crate::{
            Span,
            attributes::{Attrlist, AttrlistContext},
            tests::prelude::*,
        };

        #[test]
        fn catalog_key_strips_the_asciidoc_extension() {
            assert_eq!(include_catalog_key("other-chapters.adoc"), "other-chapters");
            assert_eq!(include_catalog_key("part1/tigers.adoc"), "part1/tigers");
            assert_eq!(include_catalog_key("../section-a.adoc"), "../section-a");
            assert_eq!(include_catalog_key("notes.txt"), "notes");

            // Only the trailing extension is removed, so a period elsewhere in
            // the name is kept.
            assert_eq!(
                include_catalog_key("using-.net-web-services.adoc"),
                "using-.net-web-services"
            );

            // Defensive fallback: production only reaches this function for a
            // target `is_asciidoc_file` accepted (a dotted name with a
            // non-empty stem), but a target without one is returned whole
            // rather than truncated.
            assert_eq!(include_catalog_key("no-extension"), "no-extension");
            assert_eq!(include_catalog_key(".adoc"), ".adoc");
        }

        fn is_full(attrlist_text: &str) -> bool {
            let parser = Parser::default();
            let span = Span::new(attrlist_text);
            let attrlist = Attrlist::parse(span, &parser, AttrlistContext::Inline)
                .item
                .item;
            is_full_include(&attrlist)
        }

        #[test]
        fn an_unfiltered_include_is_full() {
            assert!(is_full(""));
        }

        #[test]
        fn a_lines_selection_is_partial() {
            assert!(!is_full("lines=1..5"));

            // An empty `lines` value selects nothing in particular, so it does
            // not make the include partial on its own.
            assert!(is_full("lines="));
        }

        #[test]
        fn a_tag_selection_is_partial_unless_it_selects_everything() {
            assert!(!is_full("tags=ch2"));
            assert!(!is_full("tag=ch2"));
            assert!(!is_full("tags=ch2;ch3"));

            // The `**` wildcard selects every line, so it is a full include.
            assert!(is_full("tags=**"));
            assert!(is_full("tag=**"));
        }

        #[test]
        fn lines_takes_precedence_over_a_whole_file_tag_selection() {
            // A `lines` selection is partial even when `tags=**` is also present
            // (`lines` wins, matching the selection the preprocessor applies).
            assert!(!is_full("lines=1..2,tags=**"));
        }
    }
}
