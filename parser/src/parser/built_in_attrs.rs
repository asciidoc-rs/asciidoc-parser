use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use crate::{
    ASCIIDOCTOR_VERSION,
    document::InterpretedValue,
    parser::{AllowableValue, AttributeValue, ModificationContext},
};

/// The built-in default value of the `iconsdir` attribute, used when neither
/// `iconsdir` nor `imagesdir` has been configured. When `imagesdir` is set to a
/// non-empty value and `iconsdir` is left at this default, the icons directory
/// is instead derived as `{imagesdir}/icons` (see
/// [`Document::parse`](crate::document::Document)).
pub(crate) const DEFAULT_ICONSDIR: &str = "./images/icons";

/// The built-in attribute table is identical for every parser, so build it
/// once and keep it in a shared `static`. A [`Parser`] does **not** copy these
/// defaults into its own [`attribute_values`] map; instead it falls back to
/// this table on a lookup miss (see [`Parser::attribute_value`]), so creating
/// or cloning a parser allocates nothing per built-in attribute. This matters
/// because [`Parser::default`] (and parser cloning, e.g. for nested AsciiDoc
/// table cells) happens frequently. A parser only materializes an entry in its
/// own map when it *overrides* or *unsets* a built-in (the per-parser entry
/// then shadows this default).
///
/// [`Parser`]: crate::Parser
/// [`Parser::default`]: crate::Parser::default
/// [`Parser::attribute_value`]: crate::Parser::attribute_value
/// [`attribute_values`]: crate::Parser
static BUILT_IN_ATTRS: LazyLock<HashMap<String, AttributeValue>> =
    LazyLock::new(build_built_in_attrs);

/// The shared value handed out for every synthesized *backend-family flag* —
/// `backend-{backend}`, `basebackend-{basebackend}`, `filetype-{filetype}`,
/// `doctype-{doctype}`, `backend-{backend}-doctype-{doctype}`, and
/// `basebackend-{basebackend}-doctype-{doctype}`. Each is defined (with an
/// empty value) only while its `{...}` component matches the document's active
/// backend / basebackend / filetype / doctype, so they are resolved on the fly
/// rather than materialized, and every active flag can hand out this one shared
/// value by reference. See [`synthesized_attr`].
///
/// They are read-only intrinsics: a flag tracks the `backend` / `doctype` state
/// automatically, so a document header or body assignment to one (e.g.
/// `:backend-html5-doctype-article: x`) is silently ignored ([`ApiOnly`] +
/// [`silent_when_locked`]) rather than being allowed to shadow the intrinsic
/// empty value. This self-protection replaces the previous scheme
/// (materialize-and-lock inside an AsciiDoc table cell), which could not follow
/// a cell's dynamically-changing doctype.
///
/// [`ApiOnly`]: ModificationContext::ApiOnly
/// [`silent_when_locked`]: AttributeValue::silent_when_locked
static DERIVED_FAMILY_FLAG: LazyLock<AttributeValue> = LazyLock::new(|| AttributeValue {
    allowable_value: AllowableValue::Any,
    modification_context: ModificationContext::ApiOnly,
    silent_when_locked: true,
    value: InterpretedValue::Value(String::new()),
});

/// The synthesized `safe-mode-{name}` flag, which is defined (with an empty
/// value) only for the active safe mode. Like the derived doctype attribute it
/// is resolved on the fly (from `safe-mode-name`) rather than materialized, so
/// the flags of the inactive modes stay genuinely absent. See
/// [`synthesized_attr`].
///
/// It is likewise a read-only intrinsic (`ApiOnly` + `silent_when_locked`): a
/// document assignment to it is silently ignored rather than shadowing the
/// intrinsic value.
static SAFE_MODE_ACTIVE_FLAG: LazyLock<AttributeValue> = LazyLock::new(|| AttributeValue {
    allowable_value: AllowableValue::Any,
    modification_context: ModificationContext::ApiOnly,
    silent_when_locked: true,
    value: InterpretedValue::Set,
});

/// The synthesized `max-attribute-value-size` default under `SafeMode::Secure`:
/// a `4096`-byte cap on resolved attribute values. It is API/CLI-only
/// (`ApiOnly`), so a document assignment to it is rejected (locked) in every
/// mode.
static MAX_ATTRIBUTE_VALUE_SIZE_SECURE_DEFAULT: LazyLock<AttributeValue> =
    LazyLock::new(|| AttributeValue {
        allowable_value: AllowableValue::Any,
        modification_context: ModificationContext::ApiOnly,
        silent_when_locked: false,
        value: InterpretedValue::Value("4096".to_owned()),
    });

/// The synthesized `max-attribute-value-size` default for any *relaxed* safe
/// mode: an explicit unset (no limit). It stays `ApiOnly` so the attribute
/// remains locked against document assignment even when it carries no default
/// value.
static MAX_ATTRIBUTE_VALUE_SIZE_RELAXED_DEFAULT: LazyLock<AttributeValue> =
    LazyLock::new(|| AttributeValue {
        allowable_value: AllowableValue::Any,
        modification_context: ModificationContext::ApiOnly,
        silent_when_locked: false,
        value: InterpretedValue::Unset,
    });

/// Returns the mode-aware built-in default for `max-attribute-value-size`: the
/// `4096` cap under `SafeMode::Secure`, or an explicit unset (no limit) for any
/// relaxed mode.
///
/// This is consulted by [`Parser::effective_attribute`] *after* the per-parser
/// attribute map, so a caller-supplied value (which is API-only and therefore
/// always lives in that map) always wins — the default never overrides an
/// explicit limit, and a `with_safe_mode` call never rewrites one.
///
/// [`Parser::effective_attribute`]: crate::Parser
pub(crate) fn max_attribute_value_size_default(is_secure: bool) -> &'static AttributeValue {
    if is_secure {
        &MAX_ATTRIBUTE_VALUE_SIZE_SECURE_DEFAULT
    } else {
        &MAX_ATTRIBUTE_VALUE_SIZE_RELAXED_DEFAULT
    }
}

/// The synthesized `user-home` intrinsic under a *server-or-greater* safe mode
/// (`SafeMode::Server` / `SafeMode::Secure`): the current directory, `.`, which
/// masks the real home path so a document cannot learn where the processor is
/// running. Like Ruby Asciidoctor it is API/CLI-only (`ApiOnly`), so a document
/// assignment to it is rejected (locked) in every mode.
static USER_HOME_MASKED: LazyLock<AttributeValue> = LazyLock::new(|| AttributeValue {
    allowable_value: AllowableValue::Any,
    modification_context: ModificationContext::ApiOnly,
    silent_when_locked: false,
    value: InterpretedValue::Value(".".to_owned()),
});

/// The synthesized `user-home` intrinsic under a *relaxed* safe mode (below
/// `SafeMode::Server`): the user's home directory, resolved once from the
/// environment. This mirrors Ruby Asciidoctor's process-wide `USER_HOME`
/// constant (`Dir.home rescue (ENV['HOME'] || Dir.pwd)`), which is likewise
/// captured a single time. See [`resolve_user_home`] for the fallback chain.
static USER_HOME_RESOLVED: LazyLock<AttributeValue> = LazyLock::new(|| AttributeValue {
    allowable_value: AllowableValue::Any,
    modification_context: ModificationContext::ApiOnly,
    silent_when_locked: false,
    value: InterpretedValue::Value(resolve_user_home()),
});

/// Best-effort resolution of the user's home directory, mirroring Ruby
/// Asciidoctor's `USER_HOME = Dir.home rescue (ENV['HOME'] || Dir.pwd)`: the
/// home directory if one can be determined, otherwise the current working
/// directory, otherwise a literal `.`.
fn resolve_user_home() -> String {
    std::env::home_dir()
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::current_dir().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_owned())
}

/// Returns the mode-aware built-in default for the `user-home` intrinsic: the
/// user's home directory when the safe mode is below `SafeMode::Server`, or the
/// masking `.` value under `Server`/`Secure`.
///
/// Like [`max_attribute_value_size_default`], this is consulted by
/// [`Parser::effective_attribute`] *after* the per-parser attribute map, so a
/// caller-supplied `user-home` (which is API-only and therefore always lives in
/// that map) always wins regardless of builder-call order, and a
/// `with_safe_mode` change never rewrites it.
///
/// [`Parser::effective_attribute`]: crate::Parser
pub(crate) fn user_home_default(is_below_server: bool) -> &'static AttributeValue {
    if is_below_server {
        &USER_HOME_RESOLVED
    } else {
        &USER_HOME_MASKED
    }
}

static BUILT_IN_DEFAULT_VALUES: LazyLock<Arc<HashMap<String, String>>> =
    LazyLock::new(|| Arc::new(build_built_in_default_values()));

/// Returns the shared built-in default for `name`, if one is defined.
pub(crate) fn built_in_attr(name: &str) -> Option<&'static AttributeValue> {
    BUILT_IN_ATTRS.get(name)
}

/// Iterates the shared built-in attribute defaults.
pub(crate) fn built_in_attrs_iter()
-> impl Iterator<Item = (&'static String, &'static AttributeValue)> {
    BUILT_IN_ATTRS.iter()
}

/// Strips the trailing version digits from a backend name to yield its
/// *basebackend* (e.g. `html5` &rarr; `html`, `docbook45` &rarr; `docbook`),
/// matching Asciidoctor's `TrailingDigitsRx` derivation.
pub(crate) fn basebackend_of(backend: &str) -> &str {
    backend.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// Derives the output *filetype* from a backend name, matching Asciidoctor's
/// `DEFAULT_EXTENSIONS` table (keyed on the basebackend): a recognized
/// basebackend maps to its file type (`docbook` &rarr; `xml`, `manpage` &rarr;
/// `man`, `asciidoc` &rarr; `adoc`), and any other basebackend is its own
/// filetype (`html` &rarr; `html`, `pdf` &rarr; `pdf`, `epub` &rarr; `epub`).
pub(crate) fn filetype_of(backend: &str) -> String {
    match basebackend_of(backend) {
        "docbook" => "xml".to_owned(),
        "manpage" => "man".to_owned(),
        "asciidoc" => "adoc".to_owned(),
        other => other.to_owned(),
    }
}

/// Reports whether `name` is a derived backend-family *value* attribute
/// (`basebackend` or `filetype`), resolved on the fly from the current
/// `backend` rather than stored in either attribute table (see
/// [`derived_backend_value`]).
pub(crate) fn is_derived_backend_value(name: &str) -> bool {
    matches!(name, "basebackend" | "filetype")
}

/// Reads the effective plain value of a *stored* attribute `key` (a per-parser
/// entry layered over the shared built-in default), or `None` if it is absent,
/// unset, or value-less.
fn stored_value(key: &str, overrides: &HashMap<String, AttributeValue>) -> Option<String> {
    overrides
        .get(key)
        .or_else(|| BUILT_IN_ATTRS.get(key))
        .and_then(|av| match &av.value {
            InterpretedValue::Value(v) => Some(v.clone()),
            _ => None,
        })
}

/// Resolves a derived backend-family *value* attribute (`basebackend` or
/// `filetype`) from the current `backend`, mirroring Asciidoctor's
/// backend-trait derivation. Returns `None` for any other name.
///
/// `overrides` is the caller's per-parser attribute map; layered over the
/// shared built-in defaults it determines the active `backend`.
///
/// When `backend` has been explicitly unset (or resolves to an empty value)
/// the whole family is treated as *absent*: this returns `None` so a reader
/// reports `basebackend` / `filetype` as unset rather than as an empty derived
/// value.
pub(crate) fn derived_backend_value(
    name: &str,
    overrides: &HashMap<String, AttributeValue>,
) -> Option<InterpretedValue> {
    let backend = stored_value("backend", overrides).filter(|b| !b.is_empty())?;
    match name {
        "basebackend" => Some(InterpretedValue::Value(basebackend_of(&backend).to_owned())),
        "filetype" => Some(InterpretedValue::Value(filetype_of(&backend))),
        _ => None,
    }
}

/// Resolves a *synthesized* attribute — one computed from other state rather
/// than stored in either attribute table — for the active document state:
///
/// * The backend-family flags `backend-{backend}`, `basebackend-{basebackend}`,
///   `filetype-{filetype}`, `doctype-{doctype}`,
///   `backend-{backend}-doctype-{doctype}`, and
///   `basebackend-{basebackend}-doctype-{doctype}` are each defined (empty)
///   only while their `{...}` component matches the active `backend` /
///   `doctype`.
/// * `safe-mode-{name}` is defined (empty) only for the active safe mode (as
///   reported by `safe-mode-name`).
///
/// `overrides` is the caller's per-parser attribute map; its entries (layered
/// over the shared built-in defaults) determine the active backend / doctype /
/// safe mode. Returns `None` for any name that is not a currently-active
/// synthesized attribute (so the inactive flags stay absent, matching
/// Asciidoctor).
pub(crate) fn synthesized_attr(
    name: &str,
    overrides: &HashMap<String, AttributeValue>,
) -> Option<&'static AttributeValue> {
    // The derived backend-family flags are all empty-valued and defined only for
    // the *active* backend / basebackend / filetype / doctype. Rather than parse
    // the queried name, compute the flag names that are currently active and
    // compare — unambiguous even where the components overlap. An explicitly
    // unset (or empty) `backend` / `doctype` contributes no flags, so the
    // backend-dependent and doctype-dependent groups are each gated on a
    // present, non-empty value.
    if name.starts_with("backend-")
        || name.starts_with("basebackend-")
        || name.starts_with("filetype-")
        || name.starts_with("doctype-")
    {
        let backend = stored_value("backend", overrides).filter(|b| !b.is_empty());
        let doctype = stored_value("doctype", overrides).filter(|d| !d.is_empty());

        let mut is_active = false;

        // `doctype-{doctype}` depends on the doctype alone.
        if let Some(doctype) = &doctype {
            is_active = name == format!("doctype-{doctype}");
        }

        // The remaining flags all depend on the backend (and its derived
        // basebackend / filetype), and the `*-doctype-*` combinations on the
        // doctype as well.
        if !is_active && let Some(backend) = &backend {
            let basebackend = basebackend_of(backend);
            let filetype = filetype_of(backend);

            is_active = name == format!("backend-{backend}")
                || name == format!("basebackend-{basebackend}")
                || name == format!("filetype-{filetype}");

            if !is_active && let Some(doctype) = &doctype {
                is_active = name == format!("backend-{backend}-doctype-{doctype}")
                    || name == format!("basebackend-{basebackend}-doctype-{doctype}");
            }
        }

        if is_active {
            return Some(&*DERIVED_FAMILY_FLAG);
        }
    }

    if let Some(suffix) = name.strip_prefix("safe-mode-")
        && matches!(suffix, "unsafe" | "safe" | "server" | "secure")
    {
        let active = stored_value("safe-mode-name", overrides).as_deref() == Some(suffix);
        return active.then(|| &*SAFE_MODE_ACTIVE_FLAG);
    }

    None
}

pub(super) fn built_in_default_values() -> Arc<HashMap<String, String>> {
    BUILT_IN_DEFAULT_VALUES.clone()
}

fn build_built_in_attrs() -> HashMap<String, AttributeValue> {
    let mut attrs: HashMap<String, AttributeValue> = HashMap::new();

    // ## Character replacement attributes
    //
    // These provide portable replacements for common typographical marks,
    // non-visible characters, escapes for characters with special meaning in
    // AsciiDoc, and passthroughs for characters that get encoded by default.
    // See the reference page:
    // `ref/asciidoc-lang/docs/modules/attributes/pages/character-replacement-ref.
    // adoc`.
    //
    // The entries below are listed in the same order they appear on that
    // reference page. The replacement values match Ruby Asciidoctor's
    // `INTRINSIC_ATTRIBUTES` table (e.g. `cpp` resolves to `C&#43;&#43;`, not a
    // literal `C++`).
    let char_replacement = |value: &str| AttributeValue {
        allowable_value: AllowableValue::Any,
        modification_context: ModificationContext::ApiOnly,
        silent_when_locked: false,
        value: InterpretedValue::Value(value.into()),
    };

    // `blank` is an alias for `empty` for those who find this terminology
    // clearer.
    attrs.insert("blank".to_owned(), char_replacement(""));
    attrs.insert("empty".to_owned(), char_replacement(""));
    attrs.insert("sp".to_owned(), char_replacement(" "));
    attrs.insert("nbsp".to_owned(), char_replacement("&#160;"));
    attrs.insert("zwsp".to_owned(), char_replacement("&#8203;"));
    attrs.insert("wj".to_owned(), char_replacement("&#8288;"));
    attrs.insert("apos".to_owned(), char_replacement("&#39;"));
    attrs.insert("quot".to_owned(), char_replacement("&#34;"));
    attrs.insert("lsquo".to_owned(), char_replacement("&#8216;"));
    attrs.insert("rsquo".to_owned(), char_replacement("&#8217;"));
    attrs.insert("ldquo".to_owned(), char_replacement("&#8220;"));
    attrs.insert("rdquo".to_owned(), char_replacement("&#8221;"));
    attrs.insert("deg".to_owned(), char_replacement("&#176;"));
    attrs.insert("plus".to_owned(), char_replacement("&#43;"));
    attrs.insert("brvbar".to_owned(), char_replacement("&#166;"));
    attrs.insert("vbar".to_owned(), char_replacement("|"));
    attrs.insert("amp".to_owned(), char_replacement("&"));
    attrs.insert("lt".to_owned(), char_replacement("<"));
    attrs.insert("gt".to_owned(), char_replacement(">"));
    attrs.insert("startsb".to_owned(), char_replacement("["));
    attrs.insert("endsb".to_owned(), char_replacement("]"));
    attrs.insert("caret".to_owned(), char_replacement("^"));
    attrs.insert("asterisk".to_owned(), char_replacement("*"));
    attrs.insert("tilde".to_owned(), char_replacement("~"));
    attrs.insert("backslash".to_owned(), char_replacement("\\"));
    attrs.insert("backtick".to_owned(), char_replacement("`"));
    attrs.insert("two-colons".to_owned(), char_replacement("::"));
    attrs.insert("two-semicolons".to_owned(), char_replacement(";;"));

    // `cpp` is deprecated in favor of `cxx`; both resolve to the same value.
    attrs.insert("cpp".to_owned(), char_replacement("C&#43;&#43;"));
    attrs.insert("cxx".to_owned(), char_replacement("C&#43;&#43;"));
    attrs.insert("pp".to_owned(), char_replacement("&#43;&#43;"));

    // ## Other predefined document attributes
    //
    // The groups below mirror the catalog in
    // `ref/asciidoc-lang/docs/modules/attributes/pages/document-attributes-ref.
    // adoc`. Order is not significant. Default values match Ruby Asciidoctor.
    //
    // Only attributes that are *set by default* are registered here, so they
    // resolve on a pristine parser. Attributes that are not set by default but
    // have a default value (the reference page's implied `(x)` and effective
    // `_empty_[=x]` values, e.g. `lang`, `toclevels`, `icons`) are recorded in
    // [`build_built_in_default_values`] instead: they stay absent (so an
    // attribute reference such as `{lang}` is treated as missing), but the
    // default value is applied when the attribute is later set with an empty
    // value.
    use InterpretedValue::{Set, Unset, Value};
    use ModificationContext::{Anywhere, ApiOnly, ApiOrHeader};

    // Holds a fixed value (`allowable_value` is `Any`).
    let any = |ctx, value| AttributeValue {
        allowable_value: AllowableValue::Any,
        modification_context: ctx,
        silent_when_locked: false,
        value,
    };

    // Set by default to a concrete `default` value. The value is stored directly
    // (not via [`build_built_in_default_values`]) so that setting the attribute
    // with an empty value overrides it with an empty value, rather than
    // re-applying the default.
    let set = |ctx, default: &str| AttributeValue {
        allowable_value: AllowableValue::Any,
        modification_context: ctx,
        silent_when_locked: false,
        value: Value(default.to_owned()),
    };

    // Set by default to an empty value (a boolean-style switch).
    let empty = |ctx, value| AttributeValue {
        allowable_value: AllowableValue::Empty,
        modification_context: ctx,
        silent_when_locked: false,
        value,
    };

    // ### Compliance attributes
    attrs.insert("attribute-missing".to_owned(), set(Anywhere, "skip"));
    attrs.insert("attribute-undefined".to_owned(), set(Anywhere, "drop-line"));

    // ### Localization and numbering attributes
    attrs.insert("appendix-caption".to_owned(), set(Anywhere, "Appendix"));
    attrs.insert("appendix-refsig".to_owned(), set(Anywhere, "Appendix"));
    attrs.insert("caution-caption".to_owned(), set(Anywhere, "Caution"));
    attrs.insert("chapter-refsig".to_owned(), set(Anywhere, "Chapter"));
    attrs.insert("example-caption".to_owned(), set(Anywhere, "Example"));
    attrs.insert("figure-caption".to_owned(), set(Anywhere, "Figure"));
    attrs.insert("important-caption".to_owned(), set(Anywhere, "Important"));
    attrs.insert(
        "last-update-label".to_owned(),
        set(ApiOrHeader, "Last updated"),
    );
    attrs.insert("note-caption".to_owned(), set(Anywhere, "Note"));
    attrs.insert("part-refsig".to_owned(), set(Anywhere, "Part"));
    attrs.insert("section-refsig".to_owned(), set(Anywhere, "Section"));
    attrs.insert("table-caption".to_owned(), set(Anywhere, "Table"));
    attrs.insert("tip-caption".to_owned(), set(Anywhere, "Tip"));
    attrs.insert(
        "toc-title".to_owned(),
        set(ApiOrHeader, "Table of Contents"),
    );
    attrs.insert("untitled-label".to_owned(), set(ApiOrHeader, "Untitled"));
    attrs.insert("version-label".to_owned(), set(ApiOrHeader, "Version"));
    attrs.insert("warning-caption".to_owned(), set(Anywhere, "Warning"));

    // ### Section title and table of contents attributes
    attrs.insert("idprefix".to_owned(), any(Anywhere, Value("_".into())));
    attrs.insert("idseparator".to_owned(), any(Anywhere, Value("_".into())));
    attrs.insert("sectids".to_owned(), empty(Anywhere, Set));
    attrs.insert("sectnums".to_owned(), empty(Anywhere, Unset));
    attrs.insert(
        "sectnumlevels".to_owned(),
        any(ApiOrHeader, Value("3".into())),
    );
    attrs.insert("toc".to_owned(), any(ApiOrHeader, Unset));

    // ### General content and formatting attributes
    //
    // The active backend defaults to `html5` (the only backend this crate
    // renders). It is a normal, unlocked attribute — settable in the header, the
    // body, or via the API, matching Asciidoctor, where `{backend}` reflects the
    // latest assignment — so its default context is `Anywhere`; an API caller
    // that wants to pin it uses `ApiOnly`. Its derived family —
    // `backend-{backend}`, `basebackend`, `basebackend-{basebackend}`,
    // `filetype`, `filetype-{filetype}`, and the `*-doctype-{doctype}` flags —
    // is synthesized on the fly from this value (see [`synthesized_attr`] and
    // [`derived_backend_value`]), so it is never materialized or kept in sync
    // when `backend` changes.
    attrs.insert("backend".to_owned(), any(Anywhere, Value("html5".into())));

    // The document type defaults to `article` and may be set in the header or
    // via the API. The derived `*-doctype-{doctype}` flags (see
    // [`synthesized_attr`]) track this value automatically.
    attrs.insert(
        "doctype".to_owned(),
        any(ApiOrHeader, Value("article".into())),
    );

    // The file extension of the output file (always begins with a period),
    // defaulting to `.html`. Docinfo file names are built from this suffix.
    attrs.insert(
        "outfilesuffix".to_owned(),
        any(ApiOrHeader, Value(".html".into())),
    );

    // `relfilesuffix` — the path suffix added to relative (inter-document)
    // xrefs — is intentionally *not* registered here. When it has not been
    // explicitly set it tracks the current value of `outfilesuffix` rather than
    // a hardcoded `.html`, since the two diverge for non-HTML backends (e.g.
    // `.xml` for DocBook). That read-only default is resolved on the fly by the
    // attribute readers (see `Parser::effective_attribute_for_read`); it stays
    // absent from this table so that, like Asciidoctor, it is genuinely unset
    // (and hence freely modifiable anywhere) until an author assigns it.
    attrs.insert("webfonts".to_owned(), empty(ApiOrHeader, Set));

    // ### Image and icon attributes
    attrs.insert("iconfont-remote".to_owned(), empty(ApiOrHeader, Set));

    // The default is `{imagesdir}/icons`; when `imagesdir` is left empty this
    // resolves to `./images/icons`. The `imagesdir`-relative derivation for a
    // non-empty `imagesdir` is applied after the header is parsed (see
    // `Document::parse`).
    attrs.insert("iconsdir".to_owned(), set(Anywhere, DEFAULT_ICONSDIR));
    attrs.insert("imagesdir".to_owned(), any(Anywhere, Set));

    // ### Source highlighting and formatting attributes
    attrs.insert("prewrap".to_owned(), empty(Anywhere, Set));

    // ### HTML styling attributes
    attrs.insert("copycss".to_owned(), any(ApiOrHeader, Set));
    attrs.insert("stylesdir".to_owned(), set(ApiOrHeader, "."));
    attrs.insert("stylesheet".to_owned(), any(ApiOrHeader, Set));

    // ### Security attributes
    attrs.insert("max-include-depth".to_owned(), set(ApiOnly, "64"));

    // NOTE: `max-attribute-value-size` is *not* registered here. Its `4096`
    // default is only in effect under `SafeMode::Secure`, so it is resolved as a
    // mode-aware synthesized attribute instead (see
    // [`max_attribute_value_size_default`] and `Parser::effective_attribute`).
    // Keeping it out of this mode-agnostic table is what lets a caller-supplied
    // limit (which, being API-only, always lives in the per-parser map) survive
    // a later `with_safe_mode` call regardless of builder-call order.

    // ### Parser intrinsic attributes
    //
    // The version of this crate, so documents can reference the parser version
    // (e.g. in `ifeval` expressions). This is the parser-specific counterpart
    // of Ruby Asciidoctor's `asciidoctor-version` intrinsic. Like the safe-mode
    // intrinsics below, it describes the processor itself, so it is locked
    // against document assignment (`ApiOnly`).
    attrs.insert(
        "asciidoc-parser-version".to_owned(),
        set(ApiOnly, env!("CARGO_PKG_VERSION")),
    );

    // The version of Ruby Asciidoctor whose behavior this crate implements, so
    // that documents written against Asciidoctor's own intrinsic (e.g.
    // `ifdef::asciidoctor-version[]` or an `ifeval` version comparison) behave
    // here as they do there. A document that needs to tell the two processors
    // apart can test `asciidoc-parser-version` above, which Asciidoctor does
    // not define. Locked against document assignment (`ApiOnly`) for the same
    // reason as its companion.
    attrs.insert(
        "asciidoctor-version".to_owned(),
        set(ApiOnly, ASCIIDOCTOR_VERSION),
    );

    // The always-set boolean flag identifying an Asciidoctor-compatible
    // processor. A document uses `ifdef::asciidoctor[]` to guard content meant
    // only for Asciidoctor (and its compatible implementations, such as this
    // crate). Like its companions above it is locked against document
    // assignment (`ApiOnly`).
    attrs.insert("asciidoctor".to_owned(), empty(ApiOnly, Set));

    // ### Safe-mode intrinsic attributes
    //
    // These describe the default safe mode (`SafeMode::Secure`).
    // `Parser::with_safe_mode` overrides `safe-mode-level` and `safe-mode-name`
    // (via `apply_safe_mode_attributes`) when the caller chooses a different
    // mode. The active `safe-mode-<name>` flag is *not* stored here: it is
    // synthesized on the fly from `safe-mode-name` (see [`synthesized_attr`]), so
    // exactly one flag is ever defined and the inactive flags stay absent.
    attrs.insert(
        "safe-mode-level".to_owned(),
        any(ApiOnly, Value("20".into())),
    );
    attrs.insert(
        "safe-mode-name".to_owned(),
        any(ApiOnly, Value("secure".into())),
    );

    // NOTE: The derived backend-family flags (`backend-{backend}`,
    // `basebackend-{basebackend}`, `filetype-{filetype}`, `doctype-{doctype}`,
    // and the `*-doctype-*` combinations) are *not* registered here, nor are the
    // derived `basebackend` / `filetype` values. They are synthesized on the fly
    // for the active `backend` / `doctype` by `Parser::attribute_value` (via
    // [`synthesized_attr`] and [`derived_backend_value`]), so they never need to
    // be materialized or kept in sync when `backend` or `doctype` changes.

    attrs
}

fn build_built_in_default_values() -> HashMap<String, String> {
    // The value assigned to a built-in attribute when it is set (or turned on)
    // with an empty value (e.g. a bare `:toc:` resolves to `auto`, and a bare
    // `:lang:` resolves to `en`).
    //
    // This map holds the defaults for attributes that are *not* set by default:
    // the reference page's "turn-on" attributes (`toc`, `sectnums`) and its
    // implied `(x)` / effective `_empty_[=x]` values. Because these attributes
    // are absent from [`build_built_in_attrs`], they are treated as missing when
    // referenced while unset; their default is applied only once they are
    // explicitly set. Attributes that *are* set by default store their value
    // directly in [`build_built_in_attrs`] and do not appear here, so setting
    // one with an empty value overrides it with an empty value.
    //
    // `docinfosubs` is intentionally omitted: its implied default of
    // `attributes` is handled where docinfo substitution is resolved (an unset
    // value means "apply attribute substitution"), so a bare `:docinfosubs:`
    // must remain empty rather than resolving to `attributes`.
    let mut defaults: HashMap<String, String> = HashMap::new();

    // Turn-on attributes (reference page section: section title / TOC).
    defaults.insert("sectnums".to_owned(), "all".to_owned());
    defaults.insert("toc".to_owned(), "auto".to_owned());
    defaults.insert("toclevels".to_owned(), "2".to_owned());

    // Numbering seeds (localization and numbering attributes).
    defaults.insert("appendix-number".to_owned(), "@".to_owned());
    defaults.insert("chapter-number".to_owned(), "0".to_owned());
    defaults.insert("example-number".to_owned(), "0".to_owned());
    defaults.insert("figure-number".to_owned(), "0".to_owned());
    defaults.insert("footnote-number".to_owned(), "0".to_owned());
    defaults.insert("listing-number".to_owned(), "0".to_owned());
    defaults.insert("table-number".to_owned(), "0".to_owned());
    defaults.insert("lang".to_owned(), "en".to_owned());
    defaults.insert("manname-title".to_owned(), "Name".to_owned());

    // General content and formatting attributes.
    defaults.insert("asset-uri-scheme".to_owned(), "https".to_owned());
    defaults.insert("docinfo".to_owned(), "private".to_owned());
    defaults.insert("eqnums".to_owned(), "AMS".to_owned());
    defaults.insert("media".to_owned(), "screen".to_owned());
    defaults.insert("pagewidth".to_owned(), "425".to_owned());
    defaults.insert("stem".to_owned(), "asciimath".to_owned());
    defaults.insert("table-frame".to_owned(), "all".to_owned());
    defaults.insert("table-grid".to_owned(), "all".to_owned());
    defaults.insert("table-stripes".to_owned(), "none".to_owned());

    // Image and icon attributes.
    defaults.insert("iconfont-name".to_owned(), "font-awesome".to_owned());
    defaults.insert("icons".to_owned(), "image".to_owned());
    defaults.insert("icontype".to_owned(), "png".to_owned());

    // Source highlighting and formatting attributes.
    defaults.insert("coderay-css".to_owned(), "class".to_owned());
    defaults.insert("coderay-linenums-mode".to_owned(), "table".to_owned());
    defaults.insert("highlightjs-theme".to_owned(), "github".to_owned());
    defaults.insert("prettify-theme".to_owned(), "prettify".to_owned());
    defaults.insert("pygments-css".to_owned(), "class".to_owned());
    defaults.insert("pygments-linenums-mode".to_owned(), "table".to_owned());
    defaults.insert("pygments-style".to_owned(), "default".to_owned());
    defaults.insert("rouge-css".to_owned(), "class".to_owned());
    defaults.insert("rouge-linenums-mode".to_owned(), "table".to_owned());
    defaults.insert("rouge-style".to_owned(), "github".to_owned());

    // HTML styling attributes.
    defaults.insert("toc-class".to_owned(), "toc".to_owned());

    // Manpage attributes.
    defaults.insert("man-linkstyle".to_owned(), "blue R <>".to_owned());

    defaults
}
