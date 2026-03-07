use crate::{blocks::IsBlock, tests::prelude::*};

track_file!("docs/modules/text/pages/troubleshoot-unconstrained-formatting.adoc");

non_normative!(
    r#"
= Troubleshoot Unconstrained Formatting Pairs

An xref:index.adoc#unconstrained[unconstrained formatting pair] is often used to format just one or a few characters in a word.

"#
);

mod use_unconstrained {
    use crate::{Parser, tests::prelude::*};

    non_normative!(
        r#"
[#use-unconstrained]
== When should I use unconstrained formatting?

Consider the following questions:

. Is there a letter, number, or underscore directly outside the opening or closing formatting marks?
. Is there a colon, semicolon, or closing curly bracket directly before the opening formatting mark?
. Is there a space directly inside of a formatting mark?
. Are you writing in a CJK langauge such as Chinese?

If you answered "`yes`" to any of these questions, you need to use an unconstrained pair.

To help you determine whether a particular syntax pattern requires an unconstrained pair versus a xref:index.adoc#constrained[constrained pair], consider the following scenarios:

.Constrained or Unconstrained?
[#constrained-or-unconstrained]
|===
|AsciiDoc |Result |Formatting Pair |Reason

"#
    );

    #[test]
    fn sarah() {
        verifies!(
            r#"
|`+Sara__h__+`
|Sara__h__
|Unconstrained
|The letter `a` is directly adjacent to the opening mark.

"#
        );

        let doc = Parser::default().parse(r#"Sara__h__"#);
        let sb = super::first_simple_block(&doc);
        assert_eq!(sb.content().rendered(), "Sara<em>h</em>");
    }

    #[test]
    fn bold() {
        verifies!(
            r#"
|`+**B**old+`
|**B**old
|Unconstrained
|The `o` is directly adjacent to the closing mark.

"#
        );

        let doc = Parser::default().parse(r#"**B**old"#);
        let sb = super::first_simple_block(&doc);
        assert_eq!(sb.content().rendered(), "<strong>B</strong>old");
    }

    #[test]
    fn endash_2016() {
        verifies!(
            r#"
|`+&ndash;**2016**+`
|&#8211;**2016**
|Unconstrained
|The `;` is directly adjacent to the opening mark.

"#
        );

        let doc = Parser::default().parse(r#"&ndash;**2016**"#);
        let sb = super::first_simple_block(&doc);
        assert_eq!(sb.content().rendered(), "&ndash;<strong>2016</strong>");
    }

    #[test]
    fn space_bold() {
        verifies!(
            r#"
|`+** bold **+`
|** bold **
|Unconstrained
|There are spaces directly inside the formatting marks.

"#
        );

        // NOTE: Inserted `xyz` prefix to prevent this being parsed as a list.
        let doc = Parser::default().parse(r#"xyz ** bold **"#);
        let sb = super::first_simple_block(&doc);
        assert_eq!(sb.content().rendered(), "xyz <strong> bold </strong>");
    }

    #[test]
    fn bold_cjk() {
        verifies!(
            r#"
|`+我喜欢**大**狗+`
|我喜欢**大**狗
|Unconstrained
|There are CJK characters directly outside the formatting marks.

"#
        );

        let doc = Parser::default().parse(r#"我喜欢**大**狗"#);
        let sb = super::first_simple_block(&doc);
        assert_eq!(sb.content().rendered(), "我喜欢<strong>大</strong>狗");
    }

    #[test]
    fn constrained_2016() {
        verifies!(
            r#"
|`+*2016*&ndash;+`
|*2016*&#8211;
|Constrained
|The adjacent `&` is not a letter, number, underscore, colon, or semicolon.

"#
        );

        let doc = Parser::default().parse(r#"*2016*&ndash;"#);
        let sb = super::first_simple_block(&doc);
        assert_eq!(sb.content().rendered(), "<strong>2016</strong>&ndash;");
    }

    #[test]
    fn nine_to_five() {
        verifies!(
            r#"
|`+*9*-to-*5*+`
|*9*-to-*5*
|Constrained
|The adjacent hyphen is not a letter, number, underscore, colon, or semicolon.
"#
        );

        let doc = Parser::default().parse(r#"*9*-to-*5*"#);
        let sb = super::first_simple_block(&doc);
        assert_eq!(
            sb.content().rendered(),
            "<strong>9</strong>-to-<strong>5</strong>"
        );
    }

    non_normative!(
        r#"
|===

"#
    );
}

mod unconstrained_pair_edge_cases {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
[#unconstrained-edge-cases]
=== Unconstrained pair edge cases

There are cases when it might seem logical to use a constrained pair, but an unconstrained pair is required.
xref:subs:index.adoc[Substitutions] may be applied by the parser before getting to the formatting marks, in which case the characters adjacent to those marks may not be what you see in the original source.

"#
    );

    #[test]
    fn end_points_example_1() {
        verifies!(
            r#"
One such example is enclosing a xref:monospace.adoc[monospace phrase] inside xref:quotation-marks-and-apostrophes.adoc[curved quotation marks], such as "```end points```".

You might start with the following syntax:

----
"`end points`"
----

That only gives you "`end points`".
The backticks contribute to making the curved quotation marks, but the word isn't rendered in monospace.

"#
        );

        let doc = Parser::default().parse(r#""`end points`""#);

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), r#"&#8220;end points&#8221;"#);

        assert!(blocks.next().is_none());
    }

    #[test]
    fn end_points_example_2() {
        verifies!(
            r#"
Adding another pair of backticks isn't enough either.

----
"``end points``"
----

The parser ignores the inner pair of backticks and interprets them as literal characters, rendering the phrase as "``end points``".

"#
        );

        let doc = Parser::default().parse(r#""``end points``""#);

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), r#"&#8220;`end points`&#8221;"#);

        assert!(blocks.next().is_none());
    }

    #[test]
    fn end_points_example_3() {
        verifies!(
            r#"
You have to use an unconstrained pair of monospace formatting marks to render the phrase in monospace and a constrained pair of backticks to render the quotation marks as curved.
That's three pairs of backticks in total.

.A monospace phrase inside curved quotation marks
----
"```end points```"
----

"#
        );

        let doc = Parser::default().parse(r#""```end points```""#);

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            r#"&#8220;<code>end points</code>&#8221;"#
        );

        assert!(blocks.next().is_none());
    }

    #[test]
    fn end_points_example_4() {
        verifies!(
            r#"
If, instead, you wanted to surround the monospace phrase with typewriter quotation marks, such as "[.code]``end points``", then you need to interrupt the curved quotation marks by applying a role to the monospace phrase or escaping the typewriter quote.
For example:

.A monospace phrase inside typewriter quotation marks
----
"[.code]``end points``" or \"``end points``"
----

"#
        );

        let doc = Parser::default().parse(r#""[.code]``end points``" or \"``end points``""#);

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            r#""<code class="code">end points</code>" or "<code>end points</code>""#
        );

        assert!(blocks.next().is_none());
    }

    #[test]
    fn posessive_monospace_phrase_example_1() {
        verifies!(
            r#"
Another example is a possessive, monospace phrase that ends in an "`s`".
In this case, you must switch the monospace phrase to unconstrained formatting.

----
The ``class```' static methods make it easy to operate
on files and directories.
----

.Rendered possessive, monospace phrase
====
The ``class```' static methods make it easy to operate on files and directories.
====

"#
        );

        let doc = Parser::default().parse(
            r#"The ``class```' static methods make it easy to operate on files and directories."#,
        );

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            r#"The <code>class</code>&#8217; static methods make it easy to operate on files and directories."#
        );

        assert!(blocks.next().is_none());
    }

    #[test]
    fn posessive_monospace_phrase_example_2() {
        verifies!(
            r#"
Alternately, you could encode the curved apostrophe directly in the AsciiDoc source to get the same result.

----
The `class`’ static methods make it easy to operate on files and directories.
----

"#
        );

        let doc = Parser::default().parse(
            r#"The `class`’ static methods make it easy to operate on files and directories."#,
        );

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            r#"The <code>class</code>’ static methods make it easy to operate on files and directories."#
        );

        assert!(blocks.next().is_none());
    }

    non_normative!(
        r#"
This situation is expected to improve in the future when the AsciiDoc language switches to using a parsing expression grammar for inline formatting instead of the current regular expression-based strategy.
For details, follow https://github.com/asciidoctor/asciidoctor/issues/61[Asciidoctor issue #61].

"#
    );
}

mod escape_unconstrained_formatting_marks {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
[#escape-unconstrained]
== Escape unconstrained formatting marks

Since unconstrained formatting marks are meant to match anywhere in the text, context free, that means you may catch them formatting text that you don't want styled sometimes.
Admittedly, these symbols are a bit tricky to type literally when the content calls for it.
But being able to do so is just a matter of knowing the tricks, which this section will cover.

"#
    );

    #[test]
    fn unexpected_output() {
        verifies!(
            r#"
Let's assume you are typing the following two lines:

----
The __kernel qualifier can be used with the __attribute__ keyword...

#`CB###2`# and #`CB###3`#
----

In the first sentence, you aren't looking for any text formatting, but you're certainly going to get it.
The processor will interpret the double underscore in front of _++__kernel++_ as an unconstrained formatting mark.
In the second sentence, you might expect _++CB###2++_ and _++CB###3++_ to be highlighted and displayed using a monospace font.
However, what you get is a scrambled mess.
The mix of constrained and unconstrained formatting marks in the line is ambiguous.

"#
        );

        let doc = Parser::default().parse("The __kernel qualifier can be used with the __attribute__ keyword...\n\n#`CB###2`# and #`CB###3`#");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            r#"The <em>kernel qualifier can be used with the </em>attribute__ keyword&#8230;&#8203;"#
        );

        let block2 = blocks.next().unwrap();
        let Block::Simple(sb2) = block2 else {
            panic!("Unexpected block type: {block2:?}");
        };

        assert_eq!(
            sb2.content().rendered(),
            r#"<mark><code>CB<mark>#2</code></mark> and <mark><code>CB</mark>#3</code></mark>"#
        );

        assert!(blocks.next().is_none());
    }

    non_normative!(
        r#"
There are two reliable solutions for escaping unconstrained formatting marks:

* use an attribute reference to insert the unconstrained formatting mark verbatim, or
* wrap the text you don't want formatted in an inline passthrough.

"#
    );

    #[test]
    fn attribute_reference() {
        verifies!(
            r#"
The attribute reference is preferred because it's the easiest to read:

----
:scores: __
:hash3: ###

The {scores}kernel qualifier can be used with the {scores}attribute{scores} keyword...

#`CB{hash3}2`# and #`CB{hash3}3`#
----

This works because xref:subs:attributes.adoc[attribute expansion] is performed after text formatting (i.e., xref:subs:quotes.adoc[quotes substitution]) in the normal substitution order.

"#
        );

        let doc = Parser::default().parse(":scores: __\n:hash3: ###\n\nThe {scores}kernel qualifier can be used with the {scores}attribute{scores} keyword...\n\n#`CB{hash3}2`# and #`CB{hash3}3`#");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            r#"The __kernel qualifier can be used with the __attribute__ keyword&#8230;&#8203;"#
        );

        let block2 = blocks.next().unwrap();
        let Block::Simple(sb2) = block2 else {
            panic!("Unexpected block type: {block2:?}");
        };

        assert_eq!(
            sb2.content().rendered(),
            r#"<mark><code>CB###2</code></mark> and <mark><code>CB###3</code></mark>"#
        );

        assert!(blocks.next().is_none());
    }

    #[test]
    fn inline_pass_escaping() {
        verifies!(
            r#"
Here's how you'd write these lines using the xref:pass:pass-macro.adoc[inline single plus macro] to escape the unconstrained formatting marks instead:

----
The +__kernel+ qualifier can be used with the +__attribute__+ keyword...

#`+CB###2+`# and #`+CB###3+`#
----

Notice the addition of the plus symbols.
Everything between the plus symbols is escaped from interpolation (attribute references, text formatting, etc.).
However, the text still receives proper output escaping for xref:subs:special-characters.adoc[HTML special characters] (e.g., `<` becomes `\&lt;`).

"#
        );

        let doc = Parser::default().parse("The +__kernel+ qualifier can be used with the +__attribute__+ keyword...\n\n#`+CB###2+`# and #`+CB###3+`#");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            r#"The __kernel qualifier can be used with the __attribute__ keyword&#8230;&#8203;"#
        );

        let block2 = blocks.next().unwrap();
        let Block::Simple(sb2) = block2 else {
            panic!("Unexpected block type: {block2:?}");
        };

        assert_eq!(
            sb2.content().rendered(),
            r#"<mark><code>CB###2</code></mark> and <mark><code>CB###3</code></mark>"#
        );

        assert!(blocks.next().is_none());
    }

    non_normative!(
        r#"
The enclosure `pass:[`+TEXT+`]` (text enclosed in pluses surrounded by backticks) is a special formatting combination in AsciiDoc.
It means to format TEXT as monospace, but don't interpolate formatting marks or attribute references in TEXT.
It's roughly equivalent to Markdown's backticks.
Since AsciiDoc offers more advanced formatting, the double enclosure is necessary.
"#
    );
}

fn first_simple_block<'src>(
    doc: &'src crate::Document<'src>,
) -> &'src crate::blocks::SimpleBlock<'src> {
    let block = doc.nested_blocks().next().unwrap();
    let crate::blocks::Block::Simple(sb) = block else {
        panic!("Wrong block type found: {block:#?}");
    };

    sb
}
