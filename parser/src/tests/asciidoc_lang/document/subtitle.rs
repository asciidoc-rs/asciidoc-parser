use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/document/pages/subtitle.adoc");

non_normative!(
    r#"
= Subtitle
//From @graphitefriction: this page has some weird complexity for such a simple thing

An optional subtitle can be appended to a xref:title.adoc[document title].

NOTE: The HTML 5 converter does not currently split the subtitle out from the document title when generating HTML from AsciiDoc.
The document title is only partitioned into a main and subtitle in the output of the DocBook, EPUB 3, and PDF converters.
However, the subtitle is still available via the API, so you could add support for it by extending the HTML 5 converter.

"#
);

mod subtitle_syntax {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
== Subtitle syntax

"#
    );

    #[test]
    fn title_and_subtitle() {
        verifies!(
            r#"
When the document title contains a colon followed by a space (i.e, `:{sp}`), the text after the final colon-space sequence is treated as a subtitle.

.A document title and subtitle
[source]
----
= Main Title: Subtitle
----

"#
        );

        let doc = Parser::default().parse("= Main Title: Subtitle");
        let header = doc.header();

        // `title` remains the full, combined title for backward compatibility.
        assert_eq!(header.title(), Some("Main Title: Subtitle"));
        assert_eq!(header.main_title(), Some("Main Title"));
        assert_eq!(header.subtitle(), Some("Subtitle"));
    }

    #[test]
    fn separator_searched_from_end() {
        verifies!(
            r#"
The separator is searched for from the end of the text.
Therefore, only the last occurrence of the separator (i.e, `:{sp}`) is used for partitioning the title.

.A document title that contains more than one colon-space sequence
[source]
----
= Main Title: Main Title Continued: Subtitle
----

"#
        );

        let doc = Parser::default().parse("= Main Title: Main Title Continued: Subtitle");
        let header = doc.header();

        assert_eq!(
            header.main_title(),
            Some("Main Title: Main Title Continued")
        );
        assert_eq!(header.subtitle(), Some("Subtitle"));
    }

    mod modify_the_title_separator {
        use crate::tests::prelude::*;

        non_normative!(
            r#"
=== Modify the title separator

"#
        );

        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/382):
        // The `separator` block attribute placed above the document title is not
        // yet supported. Block attribute lines in the document header are not
        // currently parsed, so the default separator is used for the example
        // below. The `title-separator` document attribute (verified further
        // down) provides the same capability.
        to_do_verifies!(
            r#"
You can change the title separator by specifying the `separator` block attribute explicitly above the document title.
A space will automatically be appended to the separator value.

.Assign separator to the document title
[source]
----
[separator=::]
= Main Title:: Subtitle
----

"#
        );

        #[test]
        fn title_separator_attribute() {
            verifies!(
                r#"
You can also assign a separator using a document attribute `title-separator` in the header.

.Assign title-separator to the document title
[source]
----
= Main Title:: Subtitle
:title-separator: ::
----

"#
            );

            // The `title-separator` attribute applies even though it is defined
            // below the document title line, because the title is partitioned
            // after the entire header has been parsed.
            let doc = Parser::default().parse("= Main Title:: Subtitle\n:title-separator: ::");
            let header = doc.header();

            assert_eq!(header.main_title(), Some("Main Title"));
            assert_eq!(header.subtitle(), Some("Subtitle"));
        }

        non_normative!(
            r#"
`title-separator` can also be assigned via the CLI.

....
$ asciidoctor -a title-separator=:: document.adoc
....

"#
        );
    }
}

// This crate always partitions the document title and exposes the result via
// `Header::main_title` and `Header::subtitle` (and `Document::subtitle`). The
// Ruby `doctitle partition:` API described below is therefore non-normative for
// this crate.
non_normative!(
    r#"
== Partition the title using the API

You can partition the title from the API when calling the `doctitle` method on Document:

.Retrieving a partitioned document title
[source,ruby]
----
title_parts = document.doctitle partition: true
puts title_parts.title
puts title_parts.subtitle
----

You can partition the title in an arbitrary way by passing the separator as a value to the partition option.
In this case, the partition option both activates subtitle partitioning and passes in a custom separator.

.Retrieving a partitioned document title with a custom separator
[source,ruby]
----
title_parts = document.doctitle partition: '::'
puts title_parts.title
puts title_parts.subtitle
----

////
.Document with a subtitle
[source]
----
include::example$title.adoc[tag=sub-1]
----

In this example, the following is true:

Main title:: The Dangerous and Thrilling Documentation Chronicles
Subtitle:: A Tale of Caffeine and Words

.Document with a subtitle and multiple colons
[source]
----
include::example$title.adoc[tag=sub-2]
----

In this example, the following is true:

Main title:: A Cautionary Tale: The Dangerous and Thrilling Documentation Chronicles
Subtitle:: A Tale of Caffeine and Words

Instead of using a colon followed by a space as the separator characters between the main title and the subtitle, you can specify a custom separator using the `title-separator` attribute.

.Document with a subtitle using a custom separator
[source]
----
include::example$title.adoc[tag=sub-3]
----

Note that a space is always appended to the value of the `title-separator` (making the default value of the `title-separator` effectively a single colon).

This content needs to be moved or reconsidered:

Asciidoctor also provides an API for extracting the title and subtitle.
See the API docs for the https://www.rubydoc.info/gems/asciidoctor/Asciidoctor/Document/Title[Document::Title] for more information.
Support for subtitle functionality for other sections is being considered.
Refer to https://github.com/asciidoctor/asciidoctor/issues/1493[Asciidoctor issue #1493].
////
"#
);
