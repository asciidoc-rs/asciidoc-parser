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
/// * `docfile` is relativized by stripping its `docdir` prefix and the
///   following separator (`docfile[(docdir.length + 1)..]`), matching
///   Asciidoctor. When no `docdir` is available to strip against, it falls back
///   to the file's base name.
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
            let relative = match raw_set_value("docdir") {
                // Strip the `docdir` prefix plus its trailing separator, matching
                // Asciidoctor's `docfile[(docdir.length + 1)..-1]`. `get` keeps
                // this panic-safe for any out-of-range or non-boundary index (a
                // `docfile` not actually under `docdir`), yielding an empty
                // relative path in that case.
                Some(docdir) => docfile.get(docdir.len() + 1..).unwrap_or("").to_owned(),
                // No `docdir` to relativize against: fall back to the base name
                // (the trailing path segment).
                None => docfile
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&docfile)
                    .to_owned(),
            };
            Some(InterpretedValue::Value(relative))
        }

        _ => None,
    }
}
