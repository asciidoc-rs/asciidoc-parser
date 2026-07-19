//! Interprets the target of a cross-reference.
//!
//! A cross-reference target is either a reference to something in the *current*
//! document (an ID or a reference text) or an [inter-document cross reference]:
//! a reference that names another document, optionally with a fragment
//! identifying an element inside it. Which of the two a target is depends on
//! whether it carries a `#`, whether it has a file extension, and – for the
//! `xref:` macro form only – whether that extension is an AsciiDoc one.
//!
//! A target that names a document brings its own destination with it, built
//! here from the path attributes in effect at the reference. That document may
//! turn out to be the one being parsed, in which case the reference points at
//! this document after all – see [`this_document_reference`].
//!
//! [inter-document cross reference]: https://docs.asciidoctor.org/asciidoc/latest/macros/inter-document-xref/

use crate::{Parser, parser::DerivedReference};

/// The file extensions an AsciiDoc processor recognizes as AsciiDoc source.
///
/// A path bearing one of these extensions names an AsciiDoc document, so the
/// extension is replaced by the output file suffix when the target is rewritten
/// to an output path.
const ASCIIDOC_EXTENSIONS: [&str; 5] = [".adoc", ".asciidoc", ".asc", ".ad", ".txt"];

/// How a cross-reference target was interpreted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum XrefTarget {
    /// A reference to the current document: the string is the ID (or reference
    /// text) to look up in the document's catalog.
    SameDocument(String),

    /// A reference to another document.
    OtherDocument {
        /// The document's path, with any AsciiDoc file extension already
        /// removed. Never empty.
        path: String,

        /// Whether the path names an AsciiDoc *source* document, i.e. whether
        /// its path is rewritten to an output path. `false` for a path that
        /// keeps a non-AsciiDoc extension (e.g. `refcard.pdf`), which is linked
        /// to as-is.
        source: bool,

        /// The fragment naming an element within that document, if the target
        /// carried one.
        fragment: Option<String>,
    },
}

/// Interprets the raw `target` of a cross-reference, mirroring Asciidoctor's
/// rules. `macro_form` distinguishes the `xref:target[]` macro from the
/// `<<target>>` shorthand: the two differ in how a target *without* a `#` is
/// read, and in which file extensions imply an AsciiDoc source document.
pub(crate) fn interpret_xref_target(target: &str, macro_form: bool) -> XrefTarget {
    // A `#` immediately preceded by an `&` is part of a numeric character
    // reference (`&#8658;`), not a document/fragment separator. As in
    // Asciidoctor, only the *first* `#` is considered: if it is disqualified
    // this way, the target is read as if it had no `#` at all.
    let hash_index = target
        .find('#')
        .filter(|index| *index == 0 || !target[..*index].ends_with('&'));

    match hash_index {
        // `path#`, `path#fragment`: always an inter-document reference.
        Some(index) if index > 0 => {
            let (path, fragment) = target.split_at(index);
            let fragment = &fragment[1..];

            let (path, source) = strip_asciidoc_extension(path, macro_form);

            XrefTarget::OtherDocument {
                path: path.to_string(),
                source,
                fragment: if fragment.is_empty() {
                    None
                } else {
                    Some(fragment.to_string())
                },
            }
        }

        // `#fragment`: an explicit same-document reference.
        Some(_) => XrefTarget::SameDocument(target[1..].to_string()),

        // No `#`. The shorthand form always reads the target as an ID, even
        // when it contains a dot; the macro form reads a target with a file
        // extension as a path.
        None if macro_form => {
            if let Some(path) = target.strip_suffix(".adoc") {
                XrefTarget::OtherDocument {
                    path: path.to_string(),
                    source: true,
                    fragment: None,
                }
            } else if has_extension(target) {
                XrefTarget::OtherDocument {
                    path: target.to_string(),
                    source: false,
                    fragment: None,
                }
            } else {
                XrefTarget::SameDocument(target.to_string())
            }
        }

        None => XrefTarget::SameDocument(target.to_string()),
    }
}

/// Removes the AsciiDoc file extension from the path portion of an
/// inter-document target, reporting whether the path names an AsciiDoc source
/// document (and is therefore rewritten to an output path).
///
/// The two forms differ in which extensions count. The `xref:` macro recognizes
/// only `.adoc`; any *other* extension is preserved as-is (`refcard.pdf`
/// remains a link to the PDF), and an extensionless path is an AsciiDoc
/// document. The shorthand form, which reaches here only when the target
/// carries a `#`, treats every AsciiDoc extension as strippable and every other
/// path as an extensionless AsciiDoc document.
fn strip_asciidoc_extension(path: &str, macro_form: bool) -> (&str, bool) {
    if macro_form {
        match path.strip_suffix(".adoc") {
            Some(stem) => (stem, true),

            // A path that carries some other extension is a reference to that
            // file, not to an AsciiDoc source document.
            None => (path, !has_extension(path)),
        }
    } else if let Some(index) = path.rfind('.')
        && ASCIIDOC_EXTENSIONS.contains(&&path[index..])
    {
        // Only the extension itself is removed, so a path that contains a
        // period elsewhere (`using-.net-web-services.adoc`) keeps it.
        (&path[..index], true)
    } else {
        // Every other path is an AsciiDoc document named without its
        // extension, whatever it may contain.
        (path, true)
    }
}

/// Reports whether `path` ends in a file extension: a period in the last path
/// segment. A period in an earlier segment (`include.d/document`) is part of a
/// directory name, not an extension.
fn has_extension(path: &str) -> bool {
    match path.rfind('.') {
        Some(index) => !path[index..].contains('/'),
        None => false,
    }
}

/// The link text used for a reference to the current document that supplied
/// none and names no element within it, when the document has neither a
/// `reftext` nor a title to name it by.
const SELF_REFERENCE_FALLBACK_TEXT: &str = "[^top]";

/// Builds the destination of a cross reference whose target names another
/// document.
///
/// The path is assembled from the document attributes in effect at the
/// reference: `relfileprefix` is prepended, and – for a path that names an
/// AsciiDoc source document – `relfilesuffix` (which falls back to
/// `outfilesuffix`) is appended in place of the extension that was stripped.
pub(crate) fn other_document_reference(
    parser: &Parser,
    path: &str,
    source: bool,
    fragment: Option<&str>,
) -> DerivedReference {
    let prefix = parser
        .attribute_value("relfileprefix")
        .as_maybe_str()
        .unwrap_or_default()
        .to_string();

    let suffix = if source {
        parser
            .attribute_value("relfilesuffix")
            .as_maybe_str()
            .unwrap_or_default()
            .to_string()
    } else {
        String::new()
    };

    let path = format!("{prefix}{path}{suffix}");

    let href = match fragment {
        Some(fragment) => format!("{path}#{fragment}"),
        None => path.clone(),
    };

    DerivedReference { href, text: path }
}

/// Builds the destination of a cross reference to the current document as a
/// whole: the top of this document, named by the document's `reftext` or, if it
/// has none, its title.
pub(crate) fn this_document_reference(parser: &Parser) -> DerivedReference {
    let reftext = parser.attribute_value("reftext");
    let doctitle = parser.attribute_value("doctitle");

    // An empty `reftext` names nothing, so it falls through to the title just
    // as an unset one does.
    let text = reftext
        .as_maybe_str()
        .filter(|reftext| !reftext.is_empty())
        .or_else(|| doctitle.as_maybe_str().filter(|title| !title.is_empty()))
        .unwrap_or(SELF_REFERENCE_FALLBACK_TEXT)
        .to_string();

    DerivedReference {
        href: "#".to_string(),
        text,
    }
}

#[cfg(test)]
mod tests {
    use super::{XrefTarget, interpret_xref_target, this_document_reference};
    use crate::{Parser, parser::ModificationContext};

    fn other(path: &str, source: bool, fragment: Option<&str>) -> XrefTarget {
        XrefTarget::OtherDocument {
            path: path.to_string(),
            source,
            fragment: fragment.map(str::to_string),
        }
    }

    #[test]
    fn shorthand_without_hash_is_same_document() {
        assert_eq!(
            interpret_xref_target("tigers", false),
            XrefTarget::SameDocument("tigers".to_string())
        );

        // Even a target that looks like a path is an ID in the shorthand form.
        assert_eq!(
            interpret_xref_target("tigers.adoc", false),
            XrefTarget::SameDocument("tigers.adoc".to_string())
        );
    }

    #[test]
    fn explicit_hash_prefix_is_same_document() {
        assert_eq!(
            interpret_xref_target("#tigers", false),
            XrefTarget::SameDocument("tigers".to_string())
        );

        assert_eq!(
            interpret_xref_target("#using-.net-web-services", true),
            XrefTarget::SameDocument("using-.net-web-services".to_string())
        );
    }

    #[test]
    fn shorthand_with_hash_is_inter_document() {
        assert_eq!(
            interpret_xref_target("tigers#", false),
            other("tigers", true, None)
        );

        assert_eq!(
            interpret_xref_target("tigers#about", false),
            other("tigers", true, Some("about"))
        );

        assert_eq!(
            interpret_xref_target("tigers.adoc#about", false),
            other("tigers", true, Some("about"))
        );

        // Only the trailing extension is removed.
        assert_eq!(
            interpret_xref_target("using-.net-web-services.adoc#", false),
            other("using-.net-web-services", true, None)
        );

        // A path with a non-AsciiDoc extension is still an AsciiDoc document in
        // the shorthand form.
        assert_eq!(
            interpret_xref_target("asciidoctor.1#", false),
            other("asciidoctor.1", true, None)
        );
    }

    #[test]
    fn macro_form_reads_extension_as_path() {
        assert_eq!(
            interpret_xref_target("tigers.adoc", true),
            other("tigers", true, None)
        );

        assert_eq!(
            interpret_xref_target("refcard.pdf", true),
            other("refcard.pdf", false, None)
        );

        // A period in an earlier path segment is not a file extension, so this
        // is an ID.
        assert_eq!(
            interpret_xref_target("sections.d/first", true),
            XrefTarget::SameDocument("sections.d/first".to_string())
        );
    }

    #[test]
    fn macro_form_with_hash_keeps_non_asciidoc_extension() {
        assert_eq!(
            interpret_xref_target("asciidoctor.1#", true),
            other("asciidoctor.1", false, None)
        );

        assert_eq!(
            interpret_xref_target("document#", true),
            other("document", true, None)
        );

        assert_eq!(
            interpret_xref_target("include.d/document#", true),
            other("include.d/document", true, None)
        );
    }

    #[test]
    fn hash_in_numeric_character_reference_is_not_a_separator() {
        // The character-replacement substitution runs before macros, so a
        // target written as `C++` arrives here carrying numeric character
        // references.
        assert_eq!(
            interpret_xref_target("C&#43;&#43;", true),
            XrefTarget::SameDocument("C&#43;&#43;".to_string())
        );

        assert_eq!(
            interpret_xref_target("Cub &#8658; Tiger", false),
            XrefTarget::SameDocument("Cub &#8658; Tiger".to_string())
        );

        // Only the first `#` decides: a later separator in a target that opens
        // with an entity is not reconsidered (matching Asciidoctor).
        assert_eq!(
            interpret_xref_target("&#43;.adoc#frag", false),
            XrefTarget::SameDocument("&#43;.adoc#frag".to_string())
        );
    }

    #[test]
    fn this_document_is_named_by_its_reftext_then_its_title() {
        // With neither, there is nothing to name the document by.
        let parser = Parser::default();
        assert_eq!(this_document_reference(&parser).href, "#");
        assert_eq!(this_document_reference(&parser).text, "[^top]");

        let parser =
            parser.with_intrinsic_attribute("doctitle", "Doc Title", ModificationContext::Anywhere);

        assert_eq!(this_document_reference(&parser).text, "Doc Title");

        // An empty `reftext` names nothing, so the title still wins.
        let parser = parser.with_intrinsic_attribute("reftext", "", ModificationContext::Anywhere);
        assert_eq!(this_document_reference(&parser).text, "Doc Title");

        let parser =
            parser.with_intrinsic_attribute("reftext", "Ref Text", ModificationContext::Anywhere);

        assert_eq!(this_document_reference(&parser).text, "Ref Text");
    }
}
