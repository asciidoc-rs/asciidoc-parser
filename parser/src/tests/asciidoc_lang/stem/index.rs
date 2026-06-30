use crate::tests::prelude::*;

track_file!("docs/modules/stem/pages/index.adoc");

non_normative!(
    r#"
= Equations and Formulas (STEM)
:page-aliases: stem.adoc
:stem: asciimath
:url-mathjax: https://www.mathjax.org
:url-asciimath: https://docs.mathjax.org/en/latest/input/asciimath.html
:url-latexmath: https://docs.mathjax.org/en/latest/input/tex/index.html
:url-mathjax-docs: https://meta.math.stackexchange.com/questions/5020/mathjax-basic-tutorial-and-quick-reference

If you need to include Science, Technology, Engineering and Math (STEM) expressions in your document, the AsciiDoc language supports embedding math-mode macros from {url-latexmath}[LaTeX] and/or {url-asciimath}[AsciiMath] notation as block or inline elements.
These elements act as passthroughs to preserve the expressions as entered.
The expressions are then passed on to the converter to be processed and rendered for display using a STEM provider (e.g., MathJax).

== Activating STEM support

To activate equation and formula support, set the `stem` attribute in the document's header (or by passing the attribute to the command line or API).

.Setting the stem attribute
[source]
----
include::example$stem.adoc[tag=base-co]
----
<.> The default notation value, `asciimath`, is assigned implicitly.

By default, AsciiDoc's stem integration assumes all equations are AsciiMath if not specified explicitly.
The HTML converter supports STEM content written in {url-asciimath}[AsciiMath^] and {url-latexmath}[TeX and LaTeX^] math notation.
The DocBook converter only processes AsciiMath notation, leaving LaTeX to be processed by a separate tool in the DocBook toolchain.

If you want to use the LaTeX notation by default, assign `latexmath` to the stem attribute.

.Assigning an alternative notation to the stem attribute
[source]
----
include::example$stem.adoc[tag=base-alt]
----

TIP: You can use both notations in the same document.
The value of the `stem` attribute merely sets the default notation.
To set the notation explicitly for a given block or inline span, just use `asciimath` or `latexmath` in place of `stem` as explained in <<mixing-notations>>.

Stem content can be displayed inline with other content or as discrete blocks.
No substitutions are applied to the content within a stem macro or block.

"#
);

mod inline_stem_content {
    use crate::{blocks::Block, tests::prelude::*};

    #[test]
    fn inline_stem_macro() {
        verifies!(
            r#"
[#inline]
== Inline STEM content

The best way to mark up an inline formula is to use the `stem` macro.
The text between the square brackets of the inline stem macro acts as an implicit passthrough.

.Inline stem macro syntax
[source#ex-inline]
----
include::example$stem.adoc[tag=in-co]
----
<.> The inline stem macro contains only one colon (`:`).
<.> Place the expression within the square brackets (`[ ]`) of the macro.

The result of <<ex-inline>> is displayed below.

====
include::example$stem.adoc[tag=in]
====

If the inline stem equation contains a right square bracket, you must escape this character using a backslash.

.Inline stem macro with a right square bracket
[source#ex-square-bracket]
----
include::example$stem.adoc[tag=in-sb]
----

The result of <<ex-square-bracket>> is displayed below.

====
include::example$stem.adoc[tag=in-sb]
====

Since the inline stem macro is an implicit passthrough, the closing square bracket it the only character you have to escape.
You don't have to worry about escaping other AsciiDoc syntax, such as attribute references.
All such syntax is passed through to the STEM processor as is.

"#
        );

        // Examples from `example$stem.adoc[tag=in]`, rendered with the default
        // `asciimath` notation. The expression is preserved verbatim and
        // surrounded by the AsciiMath inline delimiters.
        let mut parser = Parser::default().with_intrinsic_attribute(
            "stem",
            "asciimath",
            ModificationContext::Anywhere,
        );

        let doc =
            parser.parse("stem:[sqrt(4) = 2]\n\nWater (stem:[H_2O]) is a critical component.");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };
        assert_eq!(sb1.content().rendered(), r"\$sqrt(4) = 2\$");

        let block2 = blocks.next().unwrap();
        let Block::Simple(sb2) = block2 else {
            panic!("Unexpected block type: {block2:?}");
        };
        assert_eq!(
            sb2.content().rendered(),
            r"Water (\$H_2O\$) is a critical component."
        );
        assert!(blocks.next().is_none());

        // The closing square bracket is escaped with a backslash
        // (`example$stem.adoc[tag=in-sb]`); no other AsciiDoc syntax needs
        // escaping inside the passthrough.
        let doc = parser.parse(r"A matrix can be written as stem:[[[a,b\],[c,d\]\]((n),(k))].");

        let block = doc.nested_blocks().next().unwrap();
        let Block::Simple(sb) = block else {
            panic!("Unexpected block type: {block:?}");
        };
        assert_eq!(
            sb.content().rendered(),
            r"A matrix can be written as \$[[a,b],[c,d]]((n),(k))\$."
        );
    }
}

mod block_stem_content {
    use crate::{blocks::ContentModel, content::SubstitutionStep, tests::prelude::*};

    #[test]
    fn block_stem_syntax() {
        verifies!(
            r#"
[#block]
== Block STEM content

Block formulas are marked up by assigning the `stem` style to a delimited passthrough block.

.Delimited stem block syntax
[source#ex-block]
----
include::example$stem.adoc[tag=bl-co]
----
<.> Assign the stem style to the passthrough block.
<.> A passthrough block is delimited by a line of four consecutive plus signs (`pass:[++++]`).

The result <<ex-block>> is rendered beautifully in the browser thanks to MathJax!

====
include::example$stem.adoc[tag=bl]
====

TIP: You don't need to add special delimiters around the expression as the {url-mathjax-docs}[MathJax documentation^] suggests.
The AsciiDoc processor handles that for you automatically!

"#
        );

        // Example from `example$stem.adoc[tag=bl]`. The `stem` style turns the
        // passthrough block into a `stem` block whose expression has only the
        // special characters substitution applied; the math delimiters are
        // added by the converter at render time.
        let doc = Parser::default().parse("[stem]\n++++\nsqrt(4) = 2\n++++");

        let block = doc.nested_blocks().next().unwrap();

        assert_eq!(block.content_model(), ContentModel::Raw);
        assert_eq!(block.raw_context().as_ref(), "stem");
        assert_eq!(block.declared_style(), Some("stem"));
        assert_eq!(block.rendered_content(), Some("sqrt(4) = 2"));
        assert_eq!(
            block.substitution_group(),
            SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters])
        );
    }
}

// The newline-handling rules for AsciiMath and LaTeX blocks describe the
// behavior of the STEM provider (MathJax) and the converter at render time, not
// the parser. They are therefore non-normative for this crate, which preserves
// the block content verbatim and defers rendering to a downstream converter.
non_normative!(
    r#"
=== Newlines in AsciiMath blocks

Newlines in an AsciiMath block are only preserved in certain circumstances.
The following examples illustrate how newlines are handled.

[%collapsible]
.Single newline not preserved
====
[listing]
----
x
y
----
====

[%collapsible]
.Single newline preserved if escaped
====
[listing]
----
x\
y
----
====

[%collapsible]
.Sequential newlines preserved if escaped
====
[listing]
----
x\
\
y
----
====

[%collapsible]
.Paragraph break preserved
====
[listing]
----
x

y
----
====

[%collapsible]
.Sequential newlines between paragraph break preserved
====
[listing]
----
x


y
----
====

The first preserved newline splits the expression into two.
Subsequent newlines get translated into a `<br>` element.

=== Newlines in LaTeX blocks

Newlines in a LaTeX block are only preserved in certain circumstances.
The following examples illustrate how newlines are handled.

[%collapsible]
.Single newline not preserved
====
[listing]
----
x
y
----
====

[%collapsible]
.Single newline preserved if escaped
====
[listing]
----
x\\
y
----
====

[%collapsible]
.Sequential newlines preserved if escaped and prefixed by null character
====
[listing]
----
x\\
~\\
y
----
====

[%collapsible]
.Paragraph break not preserved
====
[listing]
----
x

y
----
====

[%collapsible]
.Paragraph break preserved if separated by newline spacer
====
[listing]
----
x
\\[1em]
y
----
====

The first preserved newline splits the expression into two.
Subsequent newlines get translated into a `<br>` element.

"#
);

mod mixing_stem_notations {
    use crate::{blocks::Block, tests::prelude::*};

    #[test]
    fn mixing_notations() {
        verifies!(
            r#"
[#mixing-notations]
== Mixing STEM notations

You can use multiple notations for STEM content within the same document by using the notation's name instead of the keyword `stem`.

For example, if you want to write an inline equation using the LaTeX notation, name the macro `latexmath`.

.Inline latexmath macro syntax
[source#ex-latexmath]
----
include::example$stem.adoc[tag=multi-l]
----

The result of <<ex-latexmath>> is displayed below.

====
include::example$stem.adoc[tag=multi-l]
====

The name that maps to the notation you want to use can also be applied to block STEM content.

.Using both asciimath and latexmath notations in a single document
[source]
----
include::example$stem.adoc[tag=multi-a]
----

Here's how the body of this example will be shown:

=====
include::example$stem.adoc[tag=multi-a-render]
=====

"#
        );

        // Inline `latexmath` macro (`example$stem.adoc[tag=multi-l]`): naming the
        // macro after the notation surrounds the expression with LaTeX inline
        // delimiters regardless of the document's default `stem` notation.
        let doc = Parser::default().parse(r"latexmath:[C = \alpha + \beta Y^{\gamma} + \epsilon]");

        let block = doc.nested_blocks().next().unwrap();
        let Block::Simple(sb) = block else {
            panic!("Unexpected block type: {block:?}");
        };
        assert_eq!(
            sb.content().rendered(),
            r"\(C = \alpha + \beta Y^{\gamma} + \epsilon\)"
        );

        // The notation name applied to block content (`tag=multi-a`): both the
        // `latexmath` and `asciimath` styles produce `stem` blocks that carry
        // their notation as the block style.
        let doc = Parser::default()
            .parse("[latexmath]\n++++\nx = y\n++++\n\n[asciimath]\n++++\nsqrt(4) = 2\n++++");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        assert_eq!(block1.raw_context().as_ref(), "stem");
        assert_eq!(block1.declared_style(), Some("latexmath"));

        let block2 = blocks.next().unwrap();
        assert_eq!(block2.raw_context().as_ref(), "stem");
        assert_eq!(block2.declared_style(), Some("asciimath"));
    }
}

// Equation numbering (the `eqnums` attribute) and equation references (`\label`
// and `\eqref`) are interpreted by the STEM provider (MathJax) at render time.
// The parser passes the expressions through verbatim, so these sections are
// non-normative for this crate.
non_normative!(
    r#"
== Equation numbering

When writing expresions in LaTeX, you can configure the STEM processor to add numbers to block equations.
To do so, set the `eqnums` attribute on the document to empty (or `AMS`).
Let's look at an example.

[source]
----
= Document Title
:stem: latexmath
:eqnums:

[stem]
++++
\begin{equation}
x = y^2
\end{equation}
++++
----

The automatic numbering is only applied when the expression is contained within an `\{equation}` container (in fact, to all such containers in a stem block).

If you want all expressions to be numbered, even those not designated as an equation, set the value of the `eqnums` attribute to `all`.
Let's look at an example.

[source]
----
= Document Title
:stem: latexmath
:eqnums: all

[stem]
++++
x = y^2
++++
----

In this case, the `\{equation}` container is not required.

== Reference equations

When writing expresions in LaTeX, you can reference an equation in a stem block using an inline stem macro.
First, you need to add a label (aka ID) to the (ideally numbered) equation in a stem block using `\label` .

[source]
----
= Document Title
:stem: latexmath
:eqnums:

[stem]
++++
\begin{equation}
\label{hypotenuse}
c = \sqrt{a^2 + b^2}
\end{equation}
++++
----

Next, we reference that equation using `\eqref`.

[source]
----
We can calculate the hypotenuse using stem:[\eqref{hypotenuse}].
----

You can also use `\eqref` within any LaTeX stem block.
"#
);
