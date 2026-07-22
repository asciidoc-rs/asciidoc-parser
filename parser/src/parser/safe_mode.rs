use crate::document::InterpretedValue;

/// Describes the safe mode under which a document is parsed and rendered.
///
/// Safe modes provide a security model that controls how much a document is
/// allowed to reach outside of itself. They mirror the safe modes defined by
/// [Ruby Asciidoctor], and the discriminant values are chosen so that the
/// modes compare in order of increasing safety (`Unsafe` < `Safe` < `Server` <
/// `Secure`). Features that could expose the host environment (for example,
/// embedding the contents of a file directly in the output) are only enabled
/// when the safe mode is below a threshold.
///
/// The default safe mode is [`SafeMode::Secure`], matching the most
/// conservative setting. A client may relax it via
/// [`Parser::with_safe_mode`](crate::Parser::with_safe_mode).
///
/// [Ruby Asciidoctor]: https://docs.asciidoctor.org/asciidoc/latest/safe-modes/
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SafeMode {
    /// A safe mode level that disables any of the security features enforced by
    /// Asciidoctor (Ruby or otherwise). This mode is intended for use when the
    /// document is entirely trusted.
    Unsafe = 0,

    /// A safe mode level that closely parallels [`Unsafe`](Self::Unsafe),
    /// except it prevents access to files which reside outside of the
    /// parent directory of the source file.
    Safe = 1,

    /// A safe mode level that disallows the document from attempting to read
    /// files from the file system and including their contents into the
    /// document. It also disables certain macros that pose a security risk.
    ///
    /// This is the most fitting safe mode for server deployments (hence the
    /// name).
    Server = 10,

    /// A safe mode level that disallows the document from attempting to read
    /// files from the file system and including their contents into the
    /// document, and it prevents access to file system paths.
    ///
    /// This mode allows the AsciiDoc document to be processed in a shared,
    /// server-side environment, such as a wiki, where the document should not
    /// be able to embed the contents of arbitrary files.
    ///
    /// This is the default safe mode.
    #[default]
    Secure = 20,
}

impl SafeMode {
    /// The lowercase name of this safe mode (`unsafe`, `safe`, `server`,
    /// `secure`).
    ///
    /// This is the value exposed through the `safe-mode-name` intrinsic
    /// attribute and is also used to build the `safe-mode-<name>` flag
    /// attribute. It matches the (lowercased) name reported by Ruby
    /// Asciidoctor.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Unsafe => "unsafe",
            Self::Safe => "safe",
            Self::Server => "server",
            Self::Secure => "secure",
        }
    }

    /// The numeric level of this safe mode (`0`, `1`, `10`, or `20`).
    ///
    /// This is the value exposed through the `safe-mode-level` intrinsic
    /// attribute. Higher numbers indicate a more restrictive (safer) mode.
    pub(crate) fn level(self) -> u8 {
        self as u8
    }
}

/// Applies Ruby Asciidoctor's `SafeMode::Server`-and-greater masking of the
/// `docdir` / `docfile` intrinsic attributes for a *read*.
///
/// Returns `Some(masked)` only when `name` is `docdir` or `docfile` and that
/// attribute is currently set to a plain value (as reported by `raw_set_value`,
/// which yields the *unmasked* stored value or `None` when the attribute is
/// unset):
///
/// * `docdir` is masked to an empty value, so the host directory never leaks
///   into rendered output.
/// * `docfile` is relativized against `docdir` (see [`relativize_docfile`]),
///   matching Asciidoctor's `docfile[(docdir.length + 1)..]` for the usual case
///   where `docfile` sits under `docdir`, and falling back to the base name
///   otherwise.
///
/// Returns `None` for any other name, and for an *unset* `docdir` / `docfile`
/// (so a reference to one still resolves as missing rather than empty). Because
/// the computation reads the *raw* stored values, the API-provided attributes
/// are left untouched — a non-`Server` parser still reads them back verbatim.
///
/// Both [`Parser`](crate::Parser) and its
/// [`ResolvedAttributes`](crate::parser::ResolvedAttributes) snapshot funnel
/// their `docdir` / `docfile` reads through this one function (after confirming
/// `safe >= SafeMode::Server`), so the two report identical values.
pub(crate) fn masked_doc_path(
    name: &str,
    raw_set_value: impl Fn(&str) -> Option<String>,
) -> Option<InterpretedValue> {
    match name {
        // `docdir` is blanked whenever it is set, regardless of its value.
        "docdir" => raw_set_value("docdir").map(|_| InterpretedValue::Value(String::new())),

        "docfile" => {
            let docfile = raw_set_value("docfile")?;
            let relative = relativize_docfile(&docfile, raw_set_value("docdir").as_deref());
            Some(InterpretedValue::Value(relative))
        }

        _ => None,
    }
}

/// Relativizes `docfile` against `docdir` for `SafeMode::Server` masking.
///
/// When `docfile` sits directly under `docdir` — i.e. it begins with the exact
/// `docdir` prefix followed by a path separator — the prefix and separator are
/// stripped, matching Ruby Asciidoctor's `docfile[(docdir.length + 1)..-1]`
/// (which keeps any intermediate sub-directories, not just the base name). This
/// is the normal case, since Asciidoctor derives `docdir` from `docfile`.
///
/// A trailing separator on `docdir` (e.g. `/some/dir/`) is ignored so the match
/// still lands on a path-component boundary and nested components are
/// preserved.
///
/// Unlike Asciidoctor, this crate exposes `docdir` and `docfile` as independent
/// API attributes, so a caller can pair them inconsistently. Rather than slice
/// at an unrelated byte offset (truncating the path, or dropping the first byte
/// when `docdir` is empty), any `docdir` that is absent, empty, or not an
/// actual prefix falls back to the file's base name (its trailing path
/// segment).
fn relativize_docfile(docfile: &str, docdir: Option<&str>) -> String {
    // Normalize away any trailing separator(s) on `docdir` so a directory
    // written as `/some/dir/` matches at the same component boundary as
    // `/some/dir`; without this the relative remainder loses its leading
    // separator and nested components would collapse to the base name.
    if let Some(docdir) = docdir.map(|d| d.trim_end_matches(['/', '\\']))
        && !docdir.is_empty()
        && let Some(rest) = docfile.strip_prefix(docdir)
        && let Some(after) = rest.strip_prefix(['/', '\\'])
    {
        return after.to_owned();
    }

    // No usable `docdir` prefix: use the base name (trailing path segment).
    docfile
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(docfile)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::relativize_docfile;

    #[test]
    fn strips_exact_docdir_prefix() {
        assert_eq!(
            relativize_docfile("/some/dir/sample.adoc", Some("/some/dir")),
            "sample.adoc"
        );
    }

    #[test]
    fn keeps_subdirectories_below_docdir() {
        assert_eq!(
            relativize_docfile("/some/dir/sub/sample.adoc", Some("/some/dir")),
            "sub/sample.adoc"
        );
    }

    #[test]
    fn strips_a_backslash_separated_prefix() {
        assert_eq!(
            relativize_docfile(r"C:\some\dir\sample.adoc", Some(r"C:\some\dir")),
            "sample.adoc"
        );
    }

    #[test]
    fn falls_back_to_base_name_when_docfile_is_not_under_docdir() {
        // A `docfile` outside `docdir` must not be truncated at an unrelated
        // offset; it relativizes to its base name instead.
        assert_eq!(
            relativize_docfile("/some/different/file.adoc", Some("/some/dir")),
            "file.adoc"
        );
    }

    #[test]
    fn falls_back_to_base_name_when_prefix_is_not_separator_aligned() {
        // `docdir` is a leading substring of `docfile` but not a path component
        // (no separator follows), so the slice would corrupt the name.
        assert_eq!(
            relativize_docfile("/some/dirfile.adoc", Some("/some/dir")),
            "dirfile.adoc"
        );
    }

    #[test]
    fn ignores_a_trailing_separator_on_docdir() {
        // A `docdir` written with a trailing separator still relativizes to the
        // same component boundary, preserving nested path components.
        assert_eq!(
            relativize_docfile("/some/dir/sub/sample.adoc", Some("/some/dir/")),
            "sub/sample.adoc"
        );
        assert_eq!(
            relativize_docfile("/some/dir/sample.adoc", Some("/some/dir/")),
            "sample.adoc"
        );
        // Multiple trailing separators, and the Windows separator, too.
        assert_eq!(
            relativize_docfile("/some/dir/sub/sample.adoc", Some("/some/dir///")),
            "sub/sample.adoc"
        );
        assert_eq!(
            relativize_docfile(r"C:\some\dir\sub\sample.adoc", Some(r"C:\some\dir\")),
            r"sub\sample.adoc"
        );
    }

    #[test]
    fn treats_empty_docdir_as_no_prefix() {
        // An empty `docdir` must not drop the first byte of `docfile`.
        assert_eq!(
            relativize_docfile("/some/dir/sample.adoc", Some("")),
            "sample.adoc"
        );
    }

    #[test]
    fn falls_back_to_base_name_without_a_docdir() {
        assert_eq!(
            relativize_docfile("/some/dir/sample.adoc", None),
            "sample.adoc"
        );
    }

    #[test]
    fn returns_a_bare_docfile_unchanged() {
        // A `docfile` with no directory component is its own base name.
        assert_eq!(relativize_docfile("sample.adoc", None), "sample.adoc");
        assert_eq!(
            relativize_docfile("sample.adoc", Some("/some/dir")),
            "sample.adoc"
        );
    }
}
