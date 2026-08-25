//! A corpus-wide differential harness for the builder's recognition **side
//! effects**.
//!
//! Recognizing a construct is not only about the bytes it renders. Four
//! passes of the string pipeline also *write down* what they saw: an
//! `image:` macro records its target in the asset catalog, a `link:`/`mailto:`
//! macro and every auto-linked URL or address record theirs, an inline anchor
//! (and a bibliography entry) registers its id in the reference catalog, and
//! an image whose `link=` names a dangerous scheme records a warning. Design
//! §5.2's step 6 has to replay all four from the tree, exactly once per parse
//! and in the string pipeline's own pass order, which is what
//! [`apply_macro_side_effects`](crate::content::inline_builder::apply_macro_side_effects)
//! is staged to do.
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

use crate::{
    Parser, Span,
    content::{
        Content, SubstitutionGroup,
        inline_builder::{apply_macro_side_effects, build},
    },
    document::RefType,
    parser::ModificationContext,
    warnings::WarningType,
};

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

    /// Substitution warnings, in the order the one shared list received them.
    warnings: Vec<WarningType>,
}

/// Snapshots everything `parser` has had written into it, draining the
/// warnings so the snapshot is total rather than incremental.
fn snapshot(parser: &Parser) -> SideEffects {
    // The two borrows are of different `RefCell`s (the catalog and the
    // warnings buffer), so they do not conflict; the catalog's is scoped
    // anyway, since its `Ref` outliving the read would be a hazard for any
    // later caller.
    let (images, links, refs) = {
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
        )
    };

    SideEffects {
        images,
        links,
        refs,
        warnings: parser
            .drain_substitution_warnings_since(0)
            .into_iter()
            .map(|warning| warning.warning)
            .collect(),
    }
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
fn side_effects_with(source: &str, configure: impl Fn() -> Parser) -> (SideEffects, SideEffects) {
    let golden_parser = configure();
    let mut content = Content::from(Span::new(source));
    SubstitutionGroup::Normal.apply_string_pipeline(&mut content, &golden_parser, None);

    let builder_parser = configure();
    let nodes = build(Span::new(source), &builder_parser, None);
    apply_macro_side_effects(&nodes, &builder_parser, Span::new(source), false);

    (snapshot(&golden_parser), snapshot(&builder_parser))
}

fn side_effects(source: &str) -> (SideEffects, SideEffects) {
    side_effects_with(source, corpus_parser)
}

impl SideEffects {
    /// `true` when this fixture wrote nothing at all — the shape that would
    /// make a comparison pass vacuously.
    fn is_empty(&self) -> bool {
        self.images.is_empty()
            && self.links.is_empty()
            && self.refs.is_empty()
            && self.warnings.is_empty()
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
];

/// A bibliography entry is not a plain fixture: the string pipeline recognizes
/// `[[[id]]]` only inside a bibliography list item, which is parser state
/// rather than source text. It gets its own configured pair.
fn bibliography_side_effects(source: &str) -> (SideEffects, SideEffects) {
    side_effects_with(source, || {
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
    let (images, links, refs, warnings) = CORPUS.iter().fold(
        (0usize, 0usize, 0usize, 0usize),
        |(images, links, refs, warnings), source| {
            let (golden, _) = side_effects(source);

            (
                images + golden.images.len(),
                links + golden.links.len(),
                refs + golden.refs.len(),
                warnings + golden.warnings.len(),
            )
        },
    );

    assert!(images > 0, "no fixture registered an image");
    assert!(links > 0, "no fixture registered a link");
    assert!(refs > 0, "no fixture registered an id");
    assert!(warnings > 0, "no fixture recorded a warning");
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
    assert_eq!(
        builder.warnings,
        [
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string()),
            WarningType::DuplicateId("dup".to_string()),
        ]
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
    // of the `link:`/`mailto:` macro pass, so `z` precedes `y` even though it is
    // written after it. `apply_macro_side_effects` reproduces that ordering
    // because it composes the families in the string pipeline's own order — the
    // property `links::apply_link_side_effects` documents for itself.
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
    // `blocks::list_item_marker::DefinedTerm::substitute`), so it builds no tree
    // and has nothing to replay from — and it stays correct only because it
    // never enters the suppression window, which lives inside `apply`. Hoisting
    // the flag to cover a whole parse rather than one pass would drop this
    // registration silently.
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
    // an explicit substitution list re-enters `SubstitutionGroup::apply` for its
    // body while the outer content's suppression window is open, and closes a
    // window of its own on the way out. Restoring the previous value is what
    // leaves the outer content suppressed for everything *after* the
    // passthrough; clearing it instead would let the string pipeline record
    // `outer.png` alongside the replay's copy.
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
