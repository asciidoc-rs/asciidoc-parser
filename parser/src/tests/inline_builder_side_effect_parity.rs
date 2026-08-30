//! A corpus-wide differential harness for the builder's recognition **side
//! effects**.
//!
//! Recognizing a construct is not only about the bytes it renders. Five
//! passes of the string pipeline also *write down* what they saw: an
//! `image:` macro records its target in the asset catalog, a `link:`/`mailto:`
//! macro and every auto-linked URL or address record theirs, an inline anchor
//! (and a bibliography entry) registers its id in the reference catalog, an
//! image whose `link=` names a dangerous scheme records a warning, and a
//! `footnote:`/`footnoteref:` macro registers its numbered entry — text and
//! deferred cross-references included — in the footnote catalog. Design
//! §5.2's step 6 has to replay the first four from the tree, exactly once per
//! parse and in the string pipeline's own pass order, which is what
//! [`apply_macro_side_effects`](crate::content::inline_builder::apply_macro_side_effects)
//! is staged to do.
//!
//! The **footnote** catalog is the one that cannot be staged, and the builder
//! has always written it during the build: a footnote's number *is* its
//! rendered marker, and a second `footnote:id[]` in the same content has to
//! find the first one's id already registered. What the entry carried was
//! another matter — until this increment it was the raw match string, in which
//! an already-recognized construct is one opaque placeholder codepoint. It is
//! compared here for the same reason the other four are (it is what the
//! pipeline wrote down), but against the build itself rather than a replay.
//!
//! Until now it was pinned only by hand-written fixtures inside its own
//! module — one per ordering rule it has to honor. The blast-radius
//! experiment recorded in the design doc (what breaks if `rendered_html()`
//! becomes the fold today?) says so in as many words: it "neither calls the
//! staged side effects nor sequences the fold against resolution". The
//! sibling harness
//! [`inline_builder_document_parity`](super::inline_builder_document_parity)
//! closed the second half of that sentence. This one closes the first.
//!
//! The discipline is design §5.3's: two independently-configured parsers see
//! the same fixture, one through the real
//! [`SubstitutionGroup::Normal`](crate::content::SubstitutionGroup) pipeline
//! and one through [`build`](crate::content::inline_builder::build) plus the
//! staged replay, and everything either side wrote down must match — the
//! catalog entries, in registration order, and the warnings, in the order one
//! shared list received them.
//!
//! The golden side is **frozen** (`snapshots/side_effects.txt`). Its source is
//! `SubstitutionGroup::apply_string_pipeline`, which design §5.2's step 6 is
//! about to delete; without the freeze every assertion below would be left
//! comparing the builder against itself the moment it went. The survey that
//! scoped that deletion named this corpus as the second of the two
//! *record-shaped* ones — a flat list of plainly serializable facts, needing a
//! codec for its own record rather than the `InlineNode` serialization the
//! tree-shaped corpora still owe. See [`frozen`] for why this one round-trips
//! the recording rather than comparing bytes, and [`key`] for why it is the
//! first corpus whose recording key is not the fixture source alone.

use crate::{
    Parser, Span,
    content::inline_builder::{
        apply_macro_side_effects, build,
        snapshot::{quote, recorded, unquote},
    },
    document::RefType,
    parser::ModificationContext,
    warnings::WarningType,
};

/// The recording this corpus's golden side is frozen into.
const RECORDING: &str = "side_effects";

/// Everything a recognition pass writes down *besides* the rendered string.
#[derive(Debug, Eq, PartialEq)]
struct SideEffects {
    /// Image targets with the `imagesdir` in force, in document order.
    images: Vec<(String, Option<String>)>,

    /// Link targets, in the string pipeline's three-pass order.
    links: Vec<String>,

    /// Registered ids with their reftext and kind, sorted by id (the
    /// reference catalog's own deterministic order).
    refs: Vec<(String, Option<String>, RefType)>,

    /// Substitution warnings, in the order the one shared list received them,
    /// each as its [`WarningType`] `Debug` spelling.
    ///
    /// A spelling rather than the value because the enum has fifty-odd
    /// variants and a recording has to be able to reconstruct whichever of
    /// them a fixture produced. `Debug` is total over the enum where a
    /// hand-written decoder would be a fifty-arm match kept in sync by hand,
    /// and it is injective for these payloads (a variant name plus `String`
    /// fields), so equality over the spellings is equality over the values.
    /// It is also what a failure message already prints, so a recording diff
    /// reads the same as an assertion failure.
    warnings: Vec<String>,

    /// Footnote catalog entries, in registration (document) order. Compared
    /// whole — index, id, text, deferred cross-references and location — since
    /// every field of one is written by the recognizing pass.
    footnotes: Vec<FootnoteRecord>,
}

/// One footnote catalog entry as this corpus compares it.
///
/// Field-by-field rather than through [`Footnote`](crate::document::Footnote)'s
/// own `Debug`, which omits `location` — and `location` is one of the five
/// facts this corpus exists to compare. Reaching for the whole-struct spelling
/// the way [`warnings`](SideEffects::warnings) does would have silently dropped
/// it from the freeze, and nothing would have failed.
#[derive(Debug, Eq, PartialEq)]
struct FootnoteRecord {
    index: String,
    id: Option<String>,
    text: String,

    /// The entry's deferred **cross-references**, as
    /// `FootnoteDeferred::xrefs`'s own `Debug` spelling — every one the
    /// footnote's text carries, in document order.
    ///
    /// A spelling here for the same reason as `warnings`: an `XrefSegment`
    /// carries seven fields, three of them resolver types of their own.
    ///
    /// This is the **segment list alone**, not `FootnoteDeferred`'s whole
    /// `Debug` (which also has a `template`): the structured
    /// `XrefTemplatePiece` template a piece-based build produces has no
    /// spelling the frozen golden — captured from the string pipeline's own
    /// in-band placeholder template — could ever hold, so comparing the two
    /// verbatim compares apples to a representation that no longer exists.
    /// The segment list, unlike the template, is unchanged in shape by that
    /// switch (an `XrefSegment` is still an `XrefSegment`), so it stays a
    /// meaningful freeze; the template's own literal bytes are still pinned,
    /// just via the entry's already-compared [`text`](FootnoteRecord::text)
    /// rather than via this field.
    deferred: Option<String>,

    location: Option<(usize, usize)>,
}

/// Snapshots everything `parser` has had written into it, draining the
/// warnings so the snapshot is total rather than incremental.
fn snapshot(parser: &Parser) -> SideEffects {
    // The two borrows are of different `RefCell`s (the catalog and the
    // warnings buffer), so they do not conflict; the catalog's is scoped
    // anyway, since its `Ref` outliving the read would be a hazard for any
    // later caller.
    let (images, links, refs, footnotes) = {
        let catalog = parser.catalog();

        (
            catalog
                .images()
                .iter()
                .map(|image| (image.target.clone(), image.imagesdir.clone()))
                .collect(),
            catalog.links().to_vec(),
            catalog
                .entries()
                .map(|(id, entry)| {
                    (
                        id.to_string(),
                        entry.reftext.clone(),
                        entry.ref_type.clone(),
                    )
                })
                .collect(),
            catalog
                .footnotes()
                .iter()
                .cloned()
                .map(|footnote| FootnoteRecord {
                    index: footnote.index,
                    id: footnote.id,
                    text: footnote.text,
                    deferred: footnote.deferred.map(|d| format!("{:?}", d.xrefs())),
                    location: footnote.location,
                })
                .collect(),
        )
    };

    SideEffects {
        images,
        links,
        refs,
        footnotes,
        warnings: parser
            .drain_substitution_warnings_since(0)
            .into_iter()
            .map(|warning| format!("{:?}", warning.warning))
            .collect(),
    }
}

/// What the **string pipeline** wrote down, read back from the recording — the
/// frozen half of every comparison in this module.
///
/// The pipeline still runs, and `recorded` still checks its answer
/// against the recorded one on every call, so nothing here is taken on trust
/// while the pipeline exists. What the freeze buys is the day it does not:
/// `apply_string_pipeline` is this corpus's only golden source, so deleting it
/// would otherwise leave every assertion below comparing the builder against
/// itself. Design §5.2's survey named this corpus as the second of the two
/// *record-shaped* ones — a flat list of plainly serializable facts, needing a
/// codec for its own record rather than an `InlineNode` serialization — and
/// this is that codec.
///
/// It is a **round trip** rather than a string comparison for the same reason
/// the passthrough record corpus's is: the assertions below read the golden's
/// structure, not its bytes. They ask each list's length
/// ([`the_sweep_reaches_every_list_a_recognition_pass_writes_to`]), whether an
/// entry's `deferred` is `Some`, what a footnote's `text` is
/// ([`two_shapes_where_a_tree_built_footnote_entry_still_diverges`]), and what
/// ids the reference catalog holds. So the recording is decoded back into a
/// `SideEffects` and every one of those reads goes on working unchanged.
fn frozen(config: &str, source: &str) -> SideEffects {
    decode(&recorded(RECORDING, &key(config, source)))
}

/// The recording key for one fixture under one parser configuration.
///
/// A plain `source` key is not enough here, and this corpus is the first to
/// need more: unlike the golden-HTML corpora, it runs the *same* source under
/// more than one configuration — `Hello, {alpha}!` is swept under both
/// `attribute-missing=warn` and `attribute-missing=drop-line`, which write
/// different warning lists. Keyed by source alone the two would collide, and
/// the store would report a `Decision::Conflict` on the second of them.
///
/// `\u{1}` separates the two halves rather than a readable bracket: fixtures
/// in [`CORPUS`] genuinely begin with `[` (`[[the-anchor]]…`), so a `[tag] `
/// prefix would be ambiguous in principle. The store quotes a key before
/// writing it, so the separator survives the file as `\u{1}` and reads
/// unambiguously in a diff.
fn key(config: &str, source: &str) -> String {
    if config.is_empty() {
        source.to_string()
    } else {
        format!("{config}\u{1}{source}")
    }
}

/// Encodes a snapshot as one physical line: every list is a decimal count
/// followed by its entries' fields, and every field is tab-separated.
///
/// A count rather than a delimiter because the record holds five
/// variable-length lists in a row and a string field can hold anything,
/// including whatever character a delimiter would have used. Counting is what
/// lets [`decode`] know where one list ends without reserving a byte the
/// document could type.
///
/// String fields go through the store's own [`quote`], which is what keeps a
/// record on one physical line: a footnote's text spans lines, a warning's
/// payload can hold a tab, and the string pipeline's own output carries
/// Private-Use-Area sentinels. `Option<String>` writes a bare `-` for `None`,
/// which cannot be mistaken for a value: a present one is always quoted, so it
/// always begins with `"`.
fn encode(effects: &SideEffects) -> String {
    let mut fields: Vec<String> = vec![];

    fields.push(effects.images.len().to_string());
    for (target, imagesdir) in &effects.images {
        fields.push(quote(target));
        fields.push(encode_option(imagesdir.as_deref()));
    }

    fields.push(effects.links.len().to_string());
    fields.extend(effects.links.iter().map(|link| quote(link)));

    fields.push(effects.refs.len().to_string());
    for (id, reftext, ref_type) in &effects.refs {
        fields.push(quote(id));
        fields.push(encode_option(reftext.as_deref()));
        fields.push(format!("{ref_type:?}"));
    }

    fields.push(effects.warnings.len().to_string());
    fields.extend(effects.warnings.iter().map(|warning| quote(warning)));

    fields.push(effects.footnotes.len().to_string());
    for footnote in &effects.footnotes {
        fields.push(quote(&footnote.index));
        fields.push(encode_option(footnote.id.as_deref()));
        fields.push(quote(&footnote.text));
        fields.push(encode_option(footnote.deferred.as_deref()));

        fields.push(match footnote.location {
            Some((offset, len)) => format!("{offset}:{len}"),
            None => "-".to_string(),
        });
    }

    fields.join("\t")
}

/// Reverses [`encode`].
fn decode(encoded: &str) -> SideEffects {
    let mut fields = Fields::new(encoded);

    let images = (0..fields.count("images"))
        .map(|_| {
            (
                decode_string(&fields.next("image target")),
                decode_option(&fields.next("imagesdir")),
            )
        })
        .collect();

    let links = (0..fields.count("links"))
        .map(|_| decode_string(&fields.next("link target")))
        .collect();

    let refs = (0..fields.count("refs"))
        .map(|_| {
            (
                decode_string(&fields.next("ref id")),
                decode_option(&fields.next("reftext")),
                decode_ref_type(&fields.next("ref type")),
            )
        })
        .collect();

    let warnings = (0..fields.count("warnings"))
        .map(|_| decode_string(&fields.next("warning")))
        .collect();

    let footnotes = (0..fields.count("footnotes"))
        .map(|_| FootnoteRecord {
            index: decode_string(&fields.next("footnote index")),
            id: decode_option(&fields.next("footnote id")),
            text: decode_string(&fields.next("footnote text")),
            deferred: decode_option(&fields.next("footnote deferred")),
            location: decode_location(&fields.next("footnote location")),
        })
        .collect();

    assert!(
        fields.exhausted(),
        "trailing fields in {RECORDING}.txt: {encoded:?}"
    );

    SideEffects {
        images,
        links,
        refs,
        warnings,
        footnotes,
    }
}

/// A left-to-right cursor over one record's tab-separated fields.
///
/// A struct rather than a pair of closures because the five lists are read in
/// sequence and each read advances the same position: two closures sharing one
/// `&mut usize` cannot both be live, which is what the borrow checker says
/// about the obvious spelling.
struct Fields<'a> {
    fields: Vec<&'a str>,
    at: usize,
}

impl<'a> Fields<'a> {
    fn new(encoded: &'a str) -> Self {
        Self {
            // `"".split('\t')` yields one empty field rather than none, which
            // `count` would then read as a malformed count. An empty encoding
            // is not reachable from `encode` — it always writes five counts —
            // so this guards a corrupted recording rather than a normal case.
            fields: if encoded.is_empty() {
                vec![]
            } else {
                encoded.split('\t').collect()
            },
            at: 0,
        }
    }

    /// The next field, named for the panic message if the record ran out.
    fn next(&mut self, what: &str) -> String {
        let field = self
            .fields
            .get(self.at)
            .unwrap_or_else(|| panic!("truncated record in {RECORDING}.txt: missing {what}"));

        self.at += 1;
        (*field).to_string()
    }

    /// The next field as a list length.
    fn count(&mut self, what: &str) -> usize {
        let field = self.next(what);

        field
            .parse()
            .unwrap_or_else(|_| panic!("bad {what} count in {RECORDING}.txt: {field:?}"))
    }

    /// Whether every field was consumed — a count that under-reads its own
    /// list leaves fields behind rather than running out, so the truncation
    /// guard above cannot catch it.
    fn exhausted(&self) -> bool {
        self.at == self.fields.len()
    }
}

/// A present string as [`quote`] writes it, `None` as a bare `-`.
fn encode_option(value: Option<&str>) -> String {
    value.map_or_else(|| "-".to_string(), quote)
}

/// Reverses [`encode_option`].
fn decode_option(field: &str) -> Option<String> {
    (field != "-").then(|| decode_string(field))
}

/// One quoted field, back to its bytes.
fn decode_string(field: &str) -> String {
    unquote(RECORDING, field)
}

/// A [`RefType`] as one unquoted field. Only three spellings exist, so this one
/// leaf keeps its real type rather than a `Debug` string — which is what lets
/// [`the_replay_is_not_a_no_op_for_any_family`] go on asserting
/// `RefType::Anchor` directly.
fn decode_ref_type(field: &str) -> RefType {
    match field {
        "RefType::Anchor" => RefType::Anchor,
        "RefType::Section" => RefType::Section,
        "RefType::Bibliography" => RefType::Bibliography,
        other => panic!("unrecognized ref type in {RECORDING}.txt: {other:?}"),
    }
}

/// A footnote's `(offset, length)` pair, or `-` for an entry whose defining
/// occurrence is not locatable in the document source.
fn decode_location(field: &str) -> Option<(usize, usize)> {
    if field == "-" {
        return None;
    }

    let (offset, len) = field
        .split_once(':')
        .unwrap_or_else(|| panic!("bad location in {RECORDING}.txt: {field:?}"));

    let parse = |part: &str| {
        part.parse()
            .unwrap_or_else(|_| panic!("bad location in {RECORDING}.txt: {field:?}"))
    };

    Some((parse(offset), parse(len)))
}

/// [`WarningType`] values in the spelling [`SideEffects::warnings`] records,
/// for an assertion that names the values rather than their encoding.
fn spellings(warnings: &[WarningType]) -> Vec<String> {
    warnings
        .iter()
        .map(|warning| format!("{warning:?}"))
        .collect()
}

/// The parser both sides of a comparison are built from, one instance each.
///
/// Asset cataloging is on: `register_image` and `register_link` are no-ops
/// without it, so a run that left it off would compare two empty lists and
/// pass for the wrong reason. The three attributes are what the corpus's
/// expanded-target fixtures reference; without them those fixtures would
/// leave an unexpanded `{...}` behind and register nothing, passing for the
/// same wrong reason.
fn corpus_parser() -> Parser {
    Parser::default()
        .with_catalog_assets(true)
        .with_intrinsic_attribute("logo", "logo.png", ModificationContext::Anywhere)
        .with_intrinsic_attribute(
            "url",
            "https://example.org/docs",
            ModificationContext::Anywhere,
        )
        .with_intrinsic_attribute(
            "link-macro",
            "link:index.html[Docs]",
            ModificationContext::Anywhere,
        )
}

/// Runs `source` through the real pipeline and through the builder's staged
/// replay, on two independent parsers, and returns what each wrote down.
///
/// `configure` is called once per side rather than sharing one `Parser`: both
/// sides register into the parser they are given, so a shared one would see
/// every entry twice and every duplicate-id warning fire spuriously. The same
/// two-independent-parsers discipline design §5.3 establishes for the fold's
/// own corpora.
fn side_effects_with(
    source: &str,
    config: &str,
    configure: impl Fn() -> Parser,
) -> (SideEffects, SideEffects) {
    let builder_parser = configure();
    let nodes = build(Span::new(source), &builder_parser, None);

    // The recognition **diagnostics** the builder recorded while recognizing,
    // moved onto the parser exactly as `SubstitutionGroup::apply` moves them:
    // before the replay, since the string pipeline raises them during its own
    // pass and ahead of the registrations. Without this the builder side would
    // report no warnings for the four classes it now owns, and every fixture
    // exercising one would compare something against nothing.
    builder_parser.push_substitution_warnings(builder_parser.drain_builder_diagnostics_since(0));

    apply_macro_side_effects(&nodes, &builder_parser, Span::new(source), false);

    (frozen(config, source), snapshot(&builder_parser))
}

fn side_effects(source: &str) -> (SideEffects, SideEffects) {
    side_effects_with(source, "", corpus_parser)
}

impl SideEffects {
    /// `true` when this fixture wrote nothing at all — the shape that would
    /// make a comparison pass vacuously.
    fn is_empty(&self) -> bool {
        self.images.is_empty()
            && self.links.is_empty()
            && self.refs.is_empty()
            && self.warnings.is_empty()
            && self.footnotes.is_empty()
    }
}

/// Every fixture the sweep drives. Each is a *single* content: the harness
/// compares one `Content`'s worth of registrations, so a fixture's own
/// left-to-right order is the only ordering under test.
const CORPUS: &[&str] = &[
    // Nothing to register at all — the negative controls for the sweep's own
    // non-vacuity guard, which requires the corpus *as a whole* to register
    // something, not every fixture in it. `icon:` is the interesting one: it
    // shares the image family's pass (and its node kind) but is *not* an
    // asset, so a replay that registered one would be caught here rather than
    // by a comparison, since the golden side registers nothing either.
    "plain text with no constructs",
    "*bold* and _em_ and `code`",
    "an icon:home[] icon",
    //
    // The image family, alone and in company.
    "an image:photo.png[Alt Text] inline",
    //
    // The four recognition **diagnostics** the builder records at its own
    // recognition site rather than replaying from the tree, because each
    // leaves nothing on the tree to replay from: a rejected `link:` stays
    // literal, an invalid substitution name is skipped, a `footnoteref:` builds
    // the same node its modern spelling does, and an undefined reference looks
    // exactly like a forward one.
    "a pass:bogus[dangerous] macro",
    "a stem:bogus,q[x + y] macro",
    "a link:javascript:alert(1)[click me] macro",
    "a footnoteref:[fnid,some text] macro",
    "a footnote:never-defined[] macro",
    //
    // Two diagnostics in one content, which is what pins their **order**: the
    // warnings ride in one list, so a builder that recognized these families in
    // a different order from the string pipeline would still record both and
    // only this fixture would notice.
    "a pass:bogus[x] and a link:javascript:alert(1)[click] together",
    // A diagnostic beside a registration, which pins the other order: the
    // string pipeline raises its warnings during the pass, ahead of what the
    // replay registers afterwards.
    "a link:javascript:alert(1)[bad] and an image:photo.png[Alt] together",
    "image:a.png[] and image:b.png[] and image:c.png[]",
    "image:a&b.png[Query] with a special in the target",
    "image:{logo}[Logo] from an expanded target",
    "image:x.png[alt,link=https://example.org] with a safe link",
    "image:x.png[alt,link=javascript:alert(1)] with a dangerous one",
    "image:x.png[alt,link=self] with the self form",
    //
    // The link family's three passes. The catalog fills in *pass* order
    // (auto/formal, then explicit macro, then bare address), not document
    // order, which is the whole reason the replay walks the tree three times.
    "A link:https://example.org[example] link.",
    "See https://example.org for details.",
    "write to doc@example.org today",
    "Visit https://example.org or link:docs.html[the docs], or write to doc@example.org.",
    "mailto:a@b.com[email me] and link:c.html[c] and https://d.example and e@f.example",
    "An angle-bracketed <https://example.org> link.",
    "link:{url}[Docs] from an expanded target",
    "{link-macro} wholly from one expansion",
    "a https://example.org[] bare-macro link",
    "visit https://example.org/path?q=1 now",
    "link:https://example.org[Example,role=external,window=_blank]",
    // A rendered span in a slot `render_link` writes out. Both spellings were
    // left *literal* by the builder until the slot rule went, so neither
    // registered its target where the string pipeline registers one.
    "link:https://example.org[Docs,role=*hl*] and more",
    "https://example.org[Docs,title=*Pause* and Resume] and more",
    //
    // Anchors and bibliography entries.
    "[[the-anchor]]Anchored paragraph.",
    "[[a,Reftext A]] and [[b]] and [[c,Reftext C]]",
    "text before anchor:named[Ref Text] and after",
    "[[mid-anchor]] after the anchor",
    "[#only-id]#text#",
    "[.role1.role2#the-id]#decorated#",
    "[#anchor]#anchored#",
    "[[dup]] and [[dup]] twice",
    //
    // More than one family in one content, where the pass order is what the
    // shared lists actually record.
    "image:a.png[] link:b.html[B] https://c.example [[anchor-id]] xref:anchor-id[]",
    "[[id]]An image:x.png[X] and a link:y.html[Y] and a bare z@example.org.",
    "image:x.png[alt,link=javascript:alert(1)] then [[dup]] and [[dup]]",
    "A footnote:[a note with image:f.png[F] and link:g.html[G] inside]",
    "a ((term)) beside image:t.png[T] and link:u.html[U]",
    "See xref:sec[the steps] and image:v.png[V] and https://w.example here.",
    "A #span with an image:s.png[S] and a link:t.html[T]# inside.",
    "[[outer]]A *bold image:b.png[B]* and a _link:l.html[L]_ run.",
    //
    // Nothing recognized is nothing registered: an escaped macro, and one
    // sealed inside a passthrough, must leave both catalogs alone. These are
    // not negative controls — each fixture also carries a live construct, so
    // an over-eager replay shows up as a length mismatch rather than as two
    // empty lists.
    "an escaped \\image:x.png[X] beside a live image:y.png[Y]",
    "an escaped \\link:x.html[X] beside a live link:y.html[Y]",
    "+++image:x.png[X]+++ sealed beside a live image:y.png[Y]",
    "+++link:x.html[X]+++ sealed beside a live link:y.html[Y]",
    "`image:x.png[X]` in monospace beside image:y.png[Y]",
    "pass:[image:x.png[X\\]] beside image:y.png[Y]",
    "an escaped \\[[x]] anchor beside a live [[y]] one",
    //
    // The three-pass link order again, this time with a macro that reaches
    // the pipeline only through an expansion — the form #1242 lifted — sitting
    // between the two spellings whose passes run before and after its own.
    "https://a.example then {link-macro} then b@example.org",
    "{link-macro} first, then https://a.example, then b@example.org",
    //
    // An `imagesdir` in force is part of what an image registration records.
    "{set:imagesdir:img}image:a.png[A] and image:b.png[B]",
    //
    // A registration reached from inside another construct's subtree.
    "link:outer.html[a label with an image:inner.png[I] in it]",
    "A footnote:[see link:f.html[F]] and a live link:g.html[G].",
    "((a term with an image:t.png[T] inside)) and image:u.png[U]",
    "((a term with a *link:t.html[T]* inside)) and link:u.html[U]",
    "((a term with an *[[t]]* anchor inside)) and [[u]] here",
    "((a term with *https://t.example* inside)) and https://u.example here",
    "((a term with an *image:t.png[T]* inside)) and image:u.png[U]",
    "((a term with a link:t.html[T] inside)) and link:u.html[U]",
    "((a term with https://t.example inside)) and https://u.example here",
    "((a term with an [[t]] anchor inside)) and [[u]] here",
    "((a term with t@example.org inside)) and u@example.org here",
    "indexterm2:[a term with an image:t.png[T] inside] and image:u.png[U]",
    "See xref:sec[a label with image:x.png[X]] here.",
    //
    // Duplicate ids across two families, and a reftext that fills the
    // catalog's reverse index as well as its forward one.
    "[[dup,Reftext]] and anchor:dup[Other Reftext]",
    "[[a,Alpha]] and [[b,Beta]] and a link:c.html[C]",
    //
    // An anchor's *reference text* is the fifth nested node list an
    // `InlineNode` holds — the one not named `children` — and a construct
    // enclosed by it hides there just as one enclosed by a visible index term
    // does. Every anchor spelling, and every family whose pass runs before the
    // anchor pass.
    "[[a,see image:t.png[T]]] and image:u.png[U]",
    "[[a,see link:t.html[T]]] and link:u.html[U]",
    "[[a,see https://t.example]] and https://u.example here",
    "[[a,see t@example.org]] and u@example.org here",
    "[[a,see *bold* text]] and image:u.png[U]",
    "anchor:a[see image:t.png[T]] and image:u.png[U]",
    "anchor:a[see link:t.html[T]] and link:u.html[U]",
    "[[[b,see image:t.png[T]]]] and image:u.png[U]",
    //
    // The **footnote** catalog. Unlike the four staged lists, an entry here is
    // written by the build itself, and it carries a *rendered* payload — the
    // footnote's text, plus the placeholder template and cross-reference
    // segments a `<<tgt>>` inside it defers. Every fixture below registers an
    // entry, so a build that stopped rendering the subtree (registering the
    // raw match string, in which an already-recognized construct is one opaque
    // placeholder codepoint) fails on the first of them.
    "A footnote:[a plain note] here.",
    "A footnote:[  spaced\nover lines  ] here.",
    "A footnote:[a \\] bracket] here.",
    "A footnote:[a & b < c > d] here.",
    "A footnote:[&copy; and \\&amp;] here.",
    "A footnote:[a -- b (C) c] here.",
    "A footnote:[{logo} expanded] here.",
    // A construct already recognized when the footnote is: the whole reason
    // the entry has to be folded rather than sliced out of the match string.
    "A footnote:[see https://github.com[GitHub]] here.",
    "A footnote:[an image:x.png[Alt] inline] here.",
    "A footnote:[an icon:home[] inline] here.",
    "A footnote:[mailto:a@b.com[write]] here.",
    "A footnote:[bare https://example.org here] here.",
    "A footnote:[`mono` and #hl# spans] here.",
    "A footnote:[kbd:[Ctrl+T]] here.",
    "A footnote:[btn:[OK] and menu:File[Save]] here.",
    "A footnote:[an [[anchor]] inside] here.",
    // The deferred half: a cross-reference inside a footnote is re-homed out
    // of the block's template onto the footnote's own, so these pin the
    // template *and* the segment list, in placeholder order.
    "A footnote:[see <<tgt>>] here.",
    "A footnote:[see <<tgt,the target>>] here.",
    "A footnote:[see <<tgt,*bold* text>>] here.",
    "A footnote:[<<a>> then <<b>> then <<c>>] here.",
    "A footnote:[<<a>>\nand <<b>>] here.",
    "A footnote:[*<<tgt>>*] here.",
    "A footnote:[a ((term with <<tgt>>)) inside] here.",
    "A footnote:[anchor:a[Ref <<tgt>>] inside] here.",
    "A footnote:[xref:tgt[label]] here.",
    "A footnote:[xref:doc.adoc#sec[]] here.",
    "A footnote:[<<tgt,>>] here.",
    "A footnote:[\\<<tgt>> escaped] here.",
    // A footnote beside one that defers nothing, so a template spliced onto
    // the wrong entry shows up as a mismatch rather than as a shifted index.
    "One footnote:[<<a>>] and another footnote:[plain] here.",
    // Ids: a defining occurrence, a later reference reusing its number, and
    // the deprecated spelling that packs both into one bracket.
    "First footnote:id2[<<b>>] then footnote:id2[] again.",
    // The deprecated spelling. Its text stays plain here on purpose: outside
    // compatibility mode this form also raises a deprecation warning quoting
    // the *matched macro*, and the two pipelines quote two different
    // placeholder alphabets for a construct inside it — a neighbouring gap in
    // the warning's payload, not in the entry, which
    // `a_compat_mode_footnoteref_registers_what_the_string_pipeline_does`
    // steps around by turning the warning off.
    "A footnoteref:[fid,text with a plain note] here.",
    "A footnoteref:[fid] alone.",
];

/// The deprecated `footnoteref:` form's own configured pair, with
/// `compat-mode` set so the deprecation warning is not raised.
///
/// The warning is what keeps a construct-bearing `footnoteref:` out of
/// [`CORPUS`]: it quotes the matched macro, and each pipeline quotes it out of
/// its own haystack, in which an already-recognized construct is a placeholder
/// — `\u{e000}0\u{e001}` (a deferred cross-reference) on the string side,
/// `\u{e0f0}` (an opaque piece) on the tree's. That is a divergence in the
/// *warning's payload*, and one the tree cannot close from its side: it has no
/// string haystack to quote. Turning the warning off is what lets the
/// registration underneath it be compared, which is this increment's subject.
fn compat_mode_side_effects(source: &str) -> (SideEffects, SideEffects) {
    side_effects_with(source, "compat-mode", || {
        corpus_parser().with_intrinsic_attribute("compat-mode", "", ModificationContext::Anywhere)
    })
}

#[test]
fn a_compat_mode_footnoteref_registers_what_the_string_pipeline_does() {
    for source in [
        "A footnoteref:[fid,text with <<tgt>>] here.",
        "A footnoteref:[fid,see https://github.com[GitHub]] here.",
        "A footnoteref:[fid,*bold* and <<a>> and <<b>>] here.",
        // A trailing comma is an empty *defining* text, not the no-comma
        // bare-reference shape (which registers nothing, and which `CORPUS`
        // already covers).
        "A footnoteref:[fid,] with an empty text.",
        "First footnoteref:[fid,<<a>>] then footnoteref:[fid] again.",
    ] {
        let (golden, builder) = compat_mode_side_effects(source);

        assert_eq!(
            golden, builder,
            "the deprecated footnote spelling diverged for {source:?}"
        );

        assert!(
            !golden.footnotes.is_empty(),
            "fixture registered no footnote: {source:?}"
        );
    }
}

#[test]
fn two_shapes_where_a_tree_built_footnote_entry_still_diverges() {
    // Pinned rather than left to be rediscovered. Neither is a gap this
    // increment opened — the entry used to be the raw match string, which
    // diverged for *every* fixture above — and neither is one it can close.

    // 1. A passthrough (or a STEM expression) inside a footnote. The string
    //    pipeline restores a passthrough *after* the macros step, over the
    //    whole block string — by which time the footnote's text has already
    //    been cut out of it, so the entry keeps a raw passthrough sentinel that
    //    no later pass will ever replace. It is one of design §4.2's three
    //    sentinel systems leaking into public API, and the tree simply has no
    //    sentinels: the passthrough is a node, so folding the subtree yields
    //    the restored text. The tree is *right* here and the string pipeline is
    //    wrong, which is why this is pinned as a divergence rather than fixed
    //    on the tree's side to match.
    for (source, string_side, tree_side) in [
        (
            "A footnote:[a +++<b>raw</b>+++ passthrough] here.",
            "a \u{96}0\u{97} passthrough",
            "a <b>raw</b> passthrough",
        ),
        ("A footnote:[pass:[<x>]] here.", "\u{96}0\u{97}", "<x>"),
        (
            "A footnote:[stem:[x < y]] here.",
            "\u{96}0\u{97}",
            "\\$x &lt; y\\$",
        ),
    ] {
        let (golden, builder) = side_effects(source);

        assert_eq!(
            golden.footnotes.first().map(|f| f.text.as_str()),
            Some(string_side),
            "the string pipeline stopped leaking a sentinel for {source:?}"
        );

        assert_eq!(
            builder.footnotes.first().map(|f| f.text.as_str()),
            Some(tree_side),
            "the tree stopped restoring the passthrough for {source:?}"
        );
    }

    // 2. A cross-reference inside a **link's display text**. The builder does
    //    not recognize one there at all — the link family escapes its text
    //    rather than substituting into it — so the tree holds `CharRef` leaves
    //    where the string pipeline holds a deferred reference. That is a
    //    recognition gap in the *link* family, which shows identically outside
    //    any footnote (`A link:x.html[<<tgt>>] here.` renders
    //    `&lt;&lt;tgt&gt;&gt;` in the flow either way it is reached); the
    //    footnote entry is only where it becomes visible in a side effect.
    let (golden, builder) = side_effects("A footnote:[link:x.html[<<tgt>>]] here.");

    assert_eq!(
        golden.footnotes.first().map(|f| f.text.as_str()),
        Some("<a href=\"x.html\"><a href=\"#tgt\">[tgt]</a></a>")
    );

    assert_eq!(
        builder.footnotes.first().map(|f| f.text.as_str()),
        Some("<a href=\"x.html\">&lt;&lt;tgt&gt;&gt;</a>")
    );

    assert!(
        golden
            .footnotes
            .first()
            .is_some_and(|f| f.deferred.is_some())
            && builder
                .footnotes
                .first()
                .is_some_and(|f| f.deferred.is_none()),
        "the link family started recognizing a cross-reference in its text"
    );
}

/// A bibliography entry is not a plain fixture: the string pipeline recognizes
/// `[[[id]]]` only inside a bibliography list item, which is parser state
/// rather than source text. It gets its own configured pair.
fn bibliography_side_effects(source: &str) -> (SideEffects, SideEffects) {
    side_effects_with(source, "bibliography", || {
        let parser = corpus_parser();
        parser.in_bibliography_list_item.set(true);
        parser
    })
}

#[test]
fn the_staged_replay_writes_what_the_string_pipeline_writes() {
    let mut wrote_nothing: Vec<&str> = vec![];

    for source in CORPUS {
        let (golden, builder) = side_effects(source);

        assert_eq!(
            golden, builder,
            "staged side effects diverged from the real pipeline for {source:?}"
        );

        if golden.is_empty() {
            wrote_nothing.push(*source);
        }
    }

    // Non-vacuity: a corpus whose fixtures all registered nothing would
    // compare empty against empty and pass without exercising the replay at
    // all. Naming the negative controls rather than counting them means a
    // fixture that *silently stops* registering — an expansion that no longer
    // reaches its macro, say — fails here instead of being absorbed.
    assert_eq!(
        wrote_nothing,
        [
            "plain text with no constructs",
            "*bold* and _em_ and `code`",
            "an icon:home[] icon",
        ]
    );
}

#[test]
fn the_sweep_reaches_every_list_a_recognition_pass_writes_to() {
    // The guard above only asks that *something* was written. This one pins
    // that each of the four lists `apply_macro_side_effects` composes is
    // actually reached, so dropping a whole family from the corpus would fail
    // here rather than quietly narrow the sweep.
    let (images, links, refs, warnings, footnotes, deferred) = CORPUS.iter().fold(
        (0usize, 0usize, 0usize, 0usize, 0usize, 0usize),
        |(images, links, refs, warnings, footnotes, deferred), source| {
            let (golden, _) = side_effects(source);

            (
                images + golden.images.len(),
                links + golden.links.len(),
                refs + golden.refs.len(),
                warnings + golden.warnings.len(),
                footnotes + golden.footnotes.len(),
                deferred
                    + golden
                        .footnotes
                        .iter()
                        .filter(|footnote| footnote.deferred.is_some())
                        .count(),
            )
        },
    );

    assert!(images > 0, "no fixture registered an image");
    assert!(links > 0, "no fixture registered a link");
    assert!(refs > 0, "no fixture registered an id");
    assert!(warnings > 0, "no fixture recorded a warning");
    assert!(footnotes > 0, "no fixture registered a footnote");

    // The deferred half of a footnote entry is its own reachability question:
    // a corpus that registered footnotes but none carrying a cross-reference
    // would compare `None` against `None` and never exercise the template or
    // the segment list at all.
    assert!(
        deferred > 0,
        "no fixture registered a footnote deferring a cross-reference"
    );
}

/// The `attribute-missing` diagnostic's own configured pair: the mode is
/// parser state rather than source text, and the default (`skip`) diagnoses
/// nothing at all.
fn missing_mode_side_effects(source: &str, mode: &str) -> (SideEffects, SideEffects) {
    side_effects_with(source, &format!("attribute-missing={mode}"), || {
        corpus_parser().with_intrinsic_attribute(
            "attribute-missing",
            mode,
            ModificationContext::Anywhere,
        )
    })
}

#[test]
fn the_attribute_missing_diagnostic_agrees_in_number_and_in_order() {
    // The fifth recognition diagnostic, and the only one whose *order* is not
    // already fixed by the pass that raises it. The splicing recursion visits a
    // `Styled` child's content before its own level, so a reference nested in a
    // span is found before one that sits earlier at the top level — while the
    // string pipeline, scanning a flat rendered string in which the span is
    // already `<strong>…</strong>`, sees them in source order. The straddling
    // fixtures below are what pin the correction; without it they compare
    // `[beta, alpha]` against `[alpha, beta]`.
    //
    // Both diagnosing modes are swept. `warn` leaves every reference literal,
    // so the *only* thing it changes is the warning list; `drop-line` removes
    // content as well, and still warns for each reference that triggered a
    // drop — including one inside a span, whose enclosing line the tree
    // deliberately does not drop (a documented divergence in output bytes that
    // is **not** a divergence in the diagnostic).
    for mode in ["warn", "drop-line"] {
        for source in [
            // One reference, the simplest possible case.
            "Hello, {alpha}!",
            // Several on one line, and across lines: source order either way.
            "{alpha} and {beta}",
            "first {alpha} line\nsecond {beta} line\nthird {gamma} here",
            // A reference nested in a span, *after* a top-level one: the
            // recursion finds it first and the sort has to put it back.
            "{alpha} *bold {beta}* {gamma}",
            "{alpha} _em {beta}_ and {gamma} then *{delta}*",
            // The same shape with the span first, which is already in the
            // order the recursion produces — the control that keeps the
            // fixture above from passing for the wrong reason.
            "*bold {alpha}* and {beta}",
            // Two references inside one span, and spans nested in spans.
            "{alpha} *a {beta} b {gamma}* c",
            "{alpha} *outer _inner {beta}_ tail* {gamma}",
            // An escaped reference is never a missing one, in either pipeline.
            "\\{alpha} but {beta}",
            // A *set* attribute beside a missing one: only the missing one is
            // diagnosed, so this fails if recognition drifts.
            "{logo} and {alpha}",
        ] {
            let (golden, builder) = missing_mode_side_effects(source, mode);

            assert_eq!(
                golden, builder,
                "attribute-missing diagnostics diverged under {mode} for {source:?}"
            );

            assert!(
                !golden.warnings.is_empty(),
                "fixture diagnosed nothing under {mode}: {source:?}"
            );
        }
    }
}

#[test]
fn a_bibliography_entry_registers_the_same_way() {
    for source in [
        "[[[gof]]] Gamma, Erich et al. _Design Patterns_.",
        "[[[gof,GoF]]] Gamma, Erich et al. _Design Patterns_.",
        "[[[gof]]] An entry with an https://example.org link.",
        "[[[gof]]] An entry with an image:cover.png[Cover].",
    ] {
        let (golden, builder) = bibliography_side_effects(source);

        assert_eq!(
            golden, builder,
            "staged bibliography side effects diverged for {source:?}"
        );

        assert!(
            golden
                .refs
                .iter()
                .any(|(_, _, ref_type)| *ref_type == RefType::Bibliography),
            "expected a bibliography entry for {source:?}"
        );
    }
}

#[test]
fn a_visible_index_terms_shown_text_is_walked_for_every_family() {
    // The gap this harness found, asserted positively rather than only as an
    // equality: a construct enclosed by a *visible* index term's shown text
    // lives in the term's own `children` subtree
    // ([`IndexTerm::children`](crate::inlines::IndexTerm::children)), which is
    // the newest of the four child-bearing node kinds and the one the three
    // side-effect walks did not descend into.
    //
    // Every family reaches such a child: the image pass because it runs
    // *before* the index-term pass, so its node is already there when the term
    // is built; the families that run *after* it because `apply_macro_families`
    // hands them the term's own children as their own level. A construct
    // inside a rendered span the term encloses is a third route, since this
    // step resolves a span's children in full before any of this level's
    // families run. All three are exercised, because all three are how a
    // registration can end up inside a term.
    //
    // A bare address gets no `*…*` row: `((a term with *t@example.org*))` is
    // not an address on *either* side, since the `>` closing `<strong>` is one
    // of `INLINE_EMAIL`'s own mismatch characters (see `apply_macro_families`'s
    // doc comment, which uses this very shape as its example).
    for (source, expected) in [
        (
            "((a term with an image:t.png[T] inside)) and image:u.png[U]",
            (vec!["t.png", "u.png"], vec![], vec![]),
        ),
        (
            "((a term with an *image:t.png[T]* inside)) and image:u.png[U]",
            (vec!["t.png", "u.png"], vec![], vec![]),
        ),
        (
            "((a term with a *link:t.html[T]* inside)) and link:u.html[U]",
            (vec![], vec!["t.html", "u.html"], vec![]),
        ),
        (
            "((a term with *https://t.example* inside)) and https://u.example here",
            (
                vec![],
                vec!["https://t.example", "https://u.example"],
                vec![],
            ),
        ),
        (
            "((a term with an *[[t]]* anchor inside)) and [[u]] here",
            (vec![], vec![], vec!["t", "u"]),
        ),
        (
            "((a term with a link:t.html[T] inside)) and link:u.html[U]",
            (vec![], vec!["t.html", "u.html"], vec![]),
        ),
        (
            "((a term with https://t.example inside)) and https://u.example here",
            (
                vec![],
                vec!["https://t.example", "https://u.example"],
                vec![],
            ),
        ),
        (
            "((a term with t@example.org inside)) and u@example.org here",
            (
                vec![],
                vec!["mailto:t@example.org", "mailto:u@example.org"],
                vec![],
            ),
        ),
        (
            "((a term with an [[t]] anchor inside)) and [[u]] here",
            (vec![], vec![], vec!["t", "u"]),
        ),
    ] {
        let (_, builder) = side_effects(source);
        let (images, links, refs) = expected;

        assert_eq!(
            builder
                .images
                .iter()
                .map(|(target, _)| target.as_str())
                .collect::<Vec<_>>(),
            images,
            "images for {source:?}"
        );
        assert_eq!(builder.links, links, "links for {source:?}");
        assert_eq!(
            builder
                .refs
                .iter()
                .map(|(id, _, _)| id.as_str())
                .collect::<Vec<_>>(),
            refs,
            "ids for {source:?}"
        );
    }
}

#[test]
fn an_anchors_reference_text_is_walked_and_registered() {
    // The reference text is the fifth nested node list an `InlineNode` holds,
    // and the only one not named `children` — so it is the one a walk written
    // by matching on `children` misses. Two things ride on it: a construct it
    // encloses must be registered like any other, and the *text itself* is
    // what the catalog holds for a cross-reference to this anchor to show, so
    // it must be the same string the string replacer registered.
    for (source, links, images, reftext) in [
        (
            "[[a,see link:t.html[T]]] and link:u.html[U]",
            vec!["t.html", "u.html"],
            vec![],
            r##"see <a href="t.html">T</a>"##,
        ),
        (
            "anchor:a[see link:t.html[T]] and link:u.html[U]",
            vec!["t.html", "u.html"],
            vec![],
            r##"see <a href="t.html">T</a>"##,
        ),
        (
            "[[a,see image:t.png[T]]] and image:u.png[U]",
            vec![],
            vec!["t.png", "u.png"],
            r##"see <span class="image"><img src="t.png" alt="T"></span>"##,
        ),
        // A reference text with nothing enclosed, so the common single-`Text`
        // shape is pinned beside the structural one: its bytes go into the
        // catalog exactly as they stand, never folded a second time.
        ("[[a,plain text]] here", vec![], vec![], "plain text"),
        ("[[a,A & B]] here", vec![], vec![], "A &amp; B"),
        ("[[a,(C) 1995]] here", vec![], vec![], "&#169; 1995"),
    ] {
        let (golden, builder) = side_effects(source);

        assert_eq!(golden, builder, "side effects diverged for {source:?}");

        assert_eq!(builder.links, links, "links for {source:?}");
        assert_eq!(
            builder
                .images
                .iter()
                .map(|(target, _)| target.as_str())
                .collect::<Vec<_>>(),
            images,
            "images for {source:?}"
        );
        assert_eq!(
            builder
                .refs
                .iter()
                .map(|(id, text, _)| (id.as_str(), text.as_deref()))
                .collect::<Vec<_>>(),
            [("a", Some(reftext))],
            "registered reference for {source:?}"
        );
    }
}

#[test]
fn the_replay_is_not_a_no_op_for_any_family() {
    // The comparison above would also be satisfied by a replay that wrote
    // *nothing* if the golden side happened to write nothing either. Assert
    // the builder side positively, family by family, on one fixture that
    // exercises all four at once.
    let (_, builder) =
        side_effects("image:x.png[alt,link=javascript:alert(1)] link:y.html[Y] [[dup]] [[dup]]");

    assert_eq!(builder.images, [("x.png".to_string(), None)]);
    assert_eq!(builder.links, ["y.html"]);
    assert_eq!(builder.refs, [("dup".to_string(), None, RefType::Anchor)]);
    // Through the same `Debug` spelling `SideEffects::warnings` records, so
    // what is asserted stays the two `WarningType` values themselves rather
    // than a pair of hand-written strings that could drift from them.
    assert_eq!(
        builder.warnings,
        spellings(&[
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string()),
            WarningType::DuplicateId("dup".to_string()),
        ])
    );
}

// The replay wired for real: `SubstitutionGroup::apply` performs these four
// from the tree, and the string pipeline's own copies are suppressed for the
// same content. What follows pins that switch through a whole parse, where the
// harness above pins the two sides against each other in isolation.

#[test]
fn a_real_parse_records_each_side_effect_exactly_once() {
    // The switch's own failure mode is not "nothing recorded" — the suite would
    // be loud about that — but "recorded twice", which a `Vec`-backed catalog
    // shows and a set-backed one would hide. Every fixture below carries a
    // construct from a different family, and each must appear once.
    let mut parser = Parser::default()
        .with_catalog_assets(true)
        .with_intrinsic_attribute_bool("experimental", true, ModificationContext::Anywhere);

    let doc = parser.parse(concat!(
        "image:x.png[X] and link:y.html[Y] and https://z.example[Z]\n",
        "\n",
        "[[anchor-id]]An anchor, and mailto:a@b.example[A].\n",
    ));

    let catalog = doc.catalog();

    let images: Vec<(String, Option<String>)> = catalog
        .images()
        .iter()
        .map(|i| (i.target.clone(), i.imagesdir.clone()))
        .collect();

    assert_eq!(
        images,
        [("x.png".to_string(), None)],
        "an image target should be catalogued once"
    );

    // Pass order, not source order: the auto-link / formal-URL pass runs ahead
    // of the `link:`/`mailto:` macro pass, so `z` precedes `y` even though it
    // is written after it. `apply_macro_side_effects` reproduces that
    // ordering because it composes the families in the string pipeline's
    // own order — the property `links::apply_link_side_effects` documents
    // for itself.
    assert_eq!(
        catalog.links().to_vec(),
        [
            "https://z.example".to_string(),
            "y.html".to_string(),
            "mailto:a@b.example".to_string(),
        ],
        "each link target should be catalogued once, in pass order"
    );

    assert!(
        catalog.refs.contains_key("anchor-id"),
        "the inline anchor's id should be registered: {:?}",
        catalog.refs
    );
}

#[test]
fn a_duplicate_id_warns_once_through_a_real_parse() {
    // The duplicate-id warning is the one side effect driven by a *failed*
    // registration, so it is the one the suppression could most easily double
    // or lose: the string replacer raises it when `register_ref` returns `Err`,
    // and a suppressed `register_ref` returns `Ok`. The replay's own call to
    // the real catalog is what must raise it instead — exactly once.
    let mut parser = Parser::default();
    let doc = parser.parse("[[dup]]One. [[dup]]Two.");

    let duplicates: Vec<_> = doc
        .warnings()
        .filter(|w| matches!(w.warning, WarningType::DuplicateId(_)))
        .collect();

    assert_eq!(
        duplicates.len(),
        1,
        "expected exactly one duplicate-id warning: {duplicates:?}"
    );
}

#[test]
fn a_description_list_term_still_registers_from_the_string_pipeline() {
    // The carve-out, pinned. A term runs the substitution steps directly rather
    // than through `SubstitutionGroup::apply` (see
    // `blocks::list_item_marker::DefinedTerm::substitute`), so it builds no
    // tree and has nothing to replay from — and it stays correct only
    // because it never enters the suppression window, which lives inside
    // `apply`. Hoisting the flag to cover a whole parse rather than one
    // pass would drop this registration silently.
    let mut parser = Parser::default();
    let doc = parser.parse("[[term-id]]A term:: its description.");

    assert!(
        doc.catalog().refs.contains_key("term-id"),
        "a term's leading anchor should still be registered: {:?}",
        doc.catalog().refs
    );
}

#[test]
fn a_passthrough_body_with_its_own_macros_registers_once() {
    // The nesting case the save-and-*restore* exists for. A `pass:` macro with
    // an explicit substitution list re-enters `SubstitutionGroup::apply` for
    // its body while the outer content's suppression window is open, and
    // closes a window of its own on the way out. Restoring the previous
    // value is what leaves the outer content suppressed for everything
    // *after* the passthrough; clearing it instead would let the string
    // pipeline record `outer.png` alongside the replay's copy.
    //
    // The body's own image is not catalogued at all, on this branch or before
    // it — a pre-existing gap in how a `pass:`-with-subs body reaches the
    // catalog, unrelated to this switch and unchanged by it. What this fixture
    // pins is the count for `outer.png`, which is what the flag's lifetime
    // decides.
    let mut parser = Parser::default().with_catalog_assets(true);

    let doc = parser.parse("pass:m[image:inner.png[I]] then image:outer.png[O]");

    let images: Vec<String> = doc
        .catalog()
        .images()
        .iter()
        .map(|i| i.target.clone())
        .collect();

    assert_eq!(
        images,
        ["outer.png".to_string()],
        "the image after a passthrough should be catalogued exactly once"
    );
}

// The callouts step's own registration, replayed from the tree — the one
// recognition side effect that is not a macro family.

#[test]
fn a_callout_list_validates_against_tree_registered_callouts() {
    // `Parser::callout_defined` is what a callout list consults when it is
    // parsed, one block after the listing that defines them. With the string
    // pipeline's `register_callout` suppressed, the numbers it finds are the
    // ones `apply_callout_side_effects` put there from the tree — so a list
    // whose items match the callouts must produce no warning, and one that
    // overshoots must still be caught.
    let mut parser = Parser::default();

    let doc = parser.parse(concat!(
        "----\n",
        "line one <1>\n",
        "line two <2>\n",
        "----\n",
        "<1> First.\n",
        "<2> Second.\n",
    ));

    let callout_warnings: Vec<_> = doc
        .warnings()
        .filter(|w| matches!(w.warning, WarningType::NoCalloutFound(_)))
        .collect();

    assert!(
        callout_warnings.is_empty(),
        "a matching callout list should not warn: {callout_warnings:?}"
    );

    // The complement: a list item with no callout behind it. This is what fails
    // if the replay stops registering, so it is the assertion that keeps the
    // pair above from passing vacuously.
    let doc = parser.parse(concat!(
        "----\n",
        "line one <1>\n",
        "----\n",
        "<1> First.\n",
        "<2> Second.\n",
    ));

    let missing: Vec<_> = doc
        .warnings()
        .filter(|w| matches!(w.warning, WarningType::NoCalloutFound(2)))
        .collect();

    assert_eq!(
        missing.len(),
        1,
        "expected one `no callout found for <2>` warning: {:?}",
        doc.warnings().collect::<Vec<_>>()
    );
}

#[test]
fn an_auto_numbered_callout_registers_its_resolved_number() {
    // `<.>` carries no number in the source; the builder resolves it to the
    // sequential value and stores that on the node, so the replay registers a
    // real number rather than re-deriving one from a counter it does not have.
    let mut parser = Parser::default();

    let doc = parser.parse(concat!(
        "----\n",
        "line one <.>\n",
        "line two <.>\n",
        "----\n",
        "<.> First.\n",
        "<.> Second.\n",
    ));

    let callout_warnings: Vec<_> = doc
        .warnings()
        .filter(|w| matches!(w.warning, WarningType::NoCalloutFound(_)))
        .collect();

    assert!(
        callout_warnings.is_empty(),
        "auto-numbered callouts should register 1 and 2: {callout_warnings:?}"
    );
}

#[test]
fn the_record_codec_round_trips_every_shape() {
    // The codec is the one part of the freeze the corpus cannot exercise
    // whole. Its fixtures produce a narrow slice of the record's shape space —
    // no fixture registers an image *and* a footnote *and* a warning at once,
    // none carries an `imagesdir` beside a `None` one in the same record, and
    // `RefType::Section` never appears at all — so the sweep below drives the
    // parts a recording has to survive rather than the parts one happens to
    // hold.
    //
    // The bar is that `decode(encode(x)) == x` for each, since that equality
    // is exactly what every assertion in this module rests on once the golden
    // side is a lookup.
    let empty = SideEffects {
        images: vec![],
        links: vec![],
        refs: vec![],
        warnings: vec![],
        footnotes: vec![],
    };

    // Every list populated at once, every `Option` in both states, every
    // `RefType`, and a footnote with and without each of its two optional
    // fields.
    let full = SideEffects {
        images: vec![
            ("a.png".to_string(), None),
            ("b.png".to_string(), Some("img".to_string())),
        ],
        links: vec!["x.html".to_string(), "mailto:a@b.example".to_string()],
        refs: vec![
            ("anchor".to_string(), None, RefType::Anchor),
            (
                "sect".to_string(),
                Some("Section One".to_string()),
                RefType::Section,
            ),
            (
                "gof".to_string(),
                Some("GoF".to_string()),
                RefType::Bibliography,
            ),
        ],
        warnings: spellings(&[
            WarningType::DuplicateId("dup".to_string()),
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string()),
        ]),
        footnotes: vec![
            FootnoteRecord {
                index: "1".to_string(),
                id: None,
                text: "plain".to_string(),
                deferred: None,
                location: None,
            },
            FootnoteRecord {
                index: "2".to_string(),
                id: Some("fid".to_string()),
                text: "with a reference".to_string(),
                deferred: Some("[]".to_string()),
                location: Some((12, 34)),
            },
        ],
    };

    // The bytes a line-based format has to survive, in every string position
    // the record has: a tab and a newline (which would split one record across
    // several lines), a quote and a backslash (which the quoting itself has to
    // escape), the `-` that marks an absent `Option` (so a *present* value
    // spelled `-` is not read back as `None`), and the Private-Use-Area
    // sentinels the string pipeline's own output carries.
    let tricky = SideEffects {
        images: vec![("a\tb.png".to_string(), Some("-".to_string()))],
        links: vec!["x\ny.html".to_string()],
        refs: vec![(
            "id\"quote".to_string(),
            Some("back\\slash".to_string()),
            RefType::Anchor,
        )],
        warnings: vec!["Warning(\"a\tb\")".to_string()],
        footnotes: vec![FootnoteRecord {
            index: "1".to_string(),
            id: Some("-".to_string()),
            text: "a \u{96}0\u{97} sentinel\nover lines".to_string(),
            deferred: Some("\u{e000}1\u{e001}".to_string()),
            location: Some((0, 0)),
        }],
    };

    for effects in [empty, full, tricky] {
        let encoded = encode(&effects);

        assert!(
            !encoded.contains('\n'),
            "a record must stay one physical line: {encoded:?}"
        );

        assert_eq!(decode(&encoded), effects, "round trip for {effects:?}");
    }
}

#[test]
fn the_record_codec_rejects_a_corrupted_recording() {
    // The panics the codec raises are its own failure surface, and each names
    // a distinct way a hand-edited recording can go wrong. Asserted rather
    // than left as unreachable defensive code: a recording *is* hand-editable,
    // and a silent mis-parse would hand a wrong golden to every assertion in
    // this module.
    for (encoded, expected) in [
        // Nothing at all, where five counts are required.
        ("", "missing images"),
        // A count that is not a number.
        ("x", "bad images count"),
        // A count that over-reads its list.
        ("1", "missing image target"),
        // A count that under-reads it, leaving fields behind.
        ("0\t0\t0\t0\t0\t\"extra\"", "trailing fields"),
        // A field that should be quoted and is not.
        ("1\tbare\t-\t0\t0\t0\t0", "unquoted field"),
        // A ref type spelling that does not exist.
        (
            "0\t0\t1\t\"id\"\t-\tRefType::Nope\t0\t0",
            "unrecognized ref type",
        ),
        // A location that is not an `offset:length` pair.
        ("0\t0\t0\t0\t1\t\"1\"\t-\t\"t\"\t-\tnope", "bad location"),
        ("0\t0\t0\t0\t1\t\"1\"\t-\t\"t\"\t-\tx:1", "bad location"),
    ] {
        let message = std::panic::catch_unwind(|| decode(encoded))
            .expect_err(&format!("{encoded:?} decoded without complaint"));

        let message = message
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| message.downcast_ref::<&str>().copied())
            .unwrap_or("");

        assert!(
            message.contains(expected),
            "{encoded:?} panicked with {message:?}, expected it to mention {expected:?}"
        );
    }
}
