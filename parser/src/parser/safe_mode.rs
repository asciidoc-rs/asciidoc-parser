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
/// # This crate performs no path-jail enforcement
///
/// Unlike Ruby Asciidoctor, this crate performs **no filesystem I/O of its
/// own**. Reading `include::` targets, images, and SVGs is delegated to the
/// client via [`IncludeFileHandler`](crate::parser::IncludeFileHandler),
/// [`ImageFileHandler`](crate::parser::ImageFileHandler), and
/// [`SvgFileHandler`](crate::parser::SvgFileHandler). As a consequence, the
/// path-traversal jail that Ruby Asciidoctor applies through
/// `PathResolver#system_path` – rejecting or clamping `../`, absolute paths,
/// `file://` URIs, and symlinks that escape a jail root – is **deliberately not
/// ported** (see [`PathResolver`](crate::parser::PathResolver)). Below
/// [`Secure`](Self::Secure), the raw include/image/SVG target is handed to the
/// client handler verbatim, with no traversal check and without communicating
/// any jail boundary.
///
/// **Enforcing a jail is therefore the client handler's responsibility.** A
/// handler that resolves untrusted targets against the filesystem must itself
/// reject `../`, absolute paths, and `file://` targets and resolve symlinks
/// against its own jail root; the safe mode alone will not do this for it.
///
/// [Ruby Asciidoctor]: https://docs.asciidoctor.org/asciidoc/latest/safe-modes/
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SafeMode {
    /// A safe mode level that disables any of the security features enforced by
    /// Asciidoctor (Ruby or otherwise). This mode is intended for use when the
    /// document is entirely trusted.
    Unsafe = 0,

    /// In Ruby Asciidoctor, this level parallels [`Unsafe`](Self::Unsafe)
    /// except that it prevents access to files which reside outside of the
    /// parent directory of the source file.
    ///
    /// **This crate does not enforce that jail.** Because path resolution is
    /// delegated to the client handlers (see the [type-level
    /// docs](SafeMode#this-crate-performs-no-path-jail-enforcement)), `Safe`
    /// currently imposes no restriction beyond [`Unsafe`](Self::Unsafe): the
    /// include/image/SVG handlers are consulted and their contents embedded
    /// exactly as under `Unsafe`, and no `../`/absolute/`file://` traversal
    /// check is applied. Keeping untrusted targets inside a directory is the
    /// handler's responsibility.
    Safe = 1,

    /// A safe mode level intended for server deployments (hence the name).
    ///
    /// In this crate, `Server` masks host-revealing intrinsic attributes so
    /// they cannot leak into rendered output: `docdir` reads as empty,
    /// `docfile` is relativized against `docdir`, and `user-home` reads as `.`
    /// rather than the real home directory.
    ///
    /// **`Server` does not by itself disable include or asset embedding.**
    /// Unlike what its name might suggest, at `Server` (and every level below
    /// [`Secure`](Self::Secure)) the include/image/SVG handlers *are* consulted
    /// and file contents *are* embedded: `include::` directives pull in file
    /// contents, `data-uri` images are base64-embedded, and inline/interactive
    /// SVGs are embedded. Disabling that embedding – and applying any path jail
    /// – happens only at [`Secure`](Self::Secure) (for embedding) or in the
    /// client handler (for the jail). A server-side integrator that must not
    /// embed arbitrary file contents should use [`Secure`](Self::Secure), not
    /// `Server`.
    Server = 10,

    /// A safe mode level that disables the embedding of file contents into the
    /// output.
    ///
    /// At `Secure` (and above), `include::` directives are converted to links
    /// to their targets rather than embedding file contents, `data-uri` image
    /// embedding is disabled, inline and interactive SVGs render as ordinary
    /// `<img>` elements, and docinfo files are ignored. This is the level at
    /// which the include/image/SVG handlers stop being consulted for embedding.
    ///
    /// This mode allows the AsciiDoc document to be processed in a shared,
    /// server-side environment, such as a wiki, where the document should not
    /// be able to embed the contents of arbitrary files. Note that `Secure`
    /// still enforces no path-traversal jail of its own (there is nothing left
    /// for a jail to guard, since embedding is off); a client that resolves
    /// targets against the filesystem at a lower safe mode must jail them
    /// itself (see the [type-level
    /// docs](SafeMode#this-crate-performs-no-path-jail-enforcement)).
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
