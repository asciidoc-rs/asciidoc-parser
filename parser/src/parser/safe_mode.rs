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
