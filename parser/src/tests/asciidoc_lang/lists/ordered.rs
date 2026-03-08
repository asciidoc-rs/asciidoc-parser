use crate::tests::prelude::*;

track_file!("docs/modules/lists/pages/ordered.adoc");

non_normative!(
    r#"
= Ordered Lists
:keywords: numbered list

"#
);

mod basic {
    use crate::{blocks::ListType, tests::prelude::*};

    non_normative!(
        r#"
== Basic ordered list

"#
    );

    #[test]
    fn numbered() {
        verifies!(
            r#"
Sometimes, we need to number the items in a list.
Instinct might tell you to prefix each item with a number, like in this next list:

----
include::example$ordered.adoc[tag=base-num]
----

"#
        );

        let doc = Parser::default().parse("1. Protons\n2. Electrons\n3. Neutrons");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::ArabicNumeral(Span {
                                data: "1.",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Protons",
                                        line: 1,
                                        col: 4,
                                        offset: 3,
                                    },
                                    rendered: "Protons",
                                },
                                source: Span {
                                    data: "Protons",
                                    line: 1,
                                    col: 4,
                                    offset: 3,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "1. Protons",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::ArabicNumeral(Span {
                                data: "2.",
                                line: 2,
                                col: 1,
                                offset: 11,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Electrons",
                                        line: 2,
                                        col: 4,
                                        offset: 14,
                                    },
                                    rendered: "Electrons",
                                },
                                source: Span {
                                    data: "Electrons",
                                    line: 2,
                                    col: 4,
                                    offset: 14,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "2. Electrons",
                                line: 2,
                                col: 1,
                                offset: 11,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::ArabicNumeral(Span {
                                data: "3.",
                                line: 3,
                                col: 1,
                                offset: 24,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Neutrons",
                                        line: 3,
                                        col: 4,
                                        offset: 27,
                                    },
                                    rendered: "Neutrons",
                                },
                                source: Span {
                                    data: "Neutrons",
                                    line: 3,
                                    col: 4,
                                    offset: 27,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "3. Neutrons",
                                line: 3,
                                col: 1,
                                offset: 24,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "1. Protons\n2. Electrons\n3. Neutrons",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "1. Protons\n2. Electrons\n3. Neutrons",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    #[test]
    fn default_numbers() {
        verifies!(
            r#"
The above works, but since the numbering is obvious, the AsciiDoc processor will insert the numbers for you if you omit them:

----
include::example$ordered.adoc[tag=base]
----

====
include::example$ordered.adoc[tag=base]
====

"#
        );

        let doc = Parser::default().parse(". Protons\n. Electrons\n. Neutrons");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Protons",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "Protons",
                                },
                                source: Span {
                                    data: "Protons",
                                    line: 1,
                                    col: 3,
                                    offset: 2,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Protons",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 2,
                                col: 1,
                                offset: 10,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Electrons",
                                        line: 2,
                                        col: 3,
                                        offset: 12,
                                    },
                                    rendered: "Electrons",
                                },
                                source: Span {
                                    data: "Electrons",
                                    line: 2,
                                    col: 3,
                                    offset: 12,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Electrons",
                                line: 2,
                                col: 1,
                                offset: 10,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 3,
                                col: 1,
                                offset: 22,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Neutrons",
                                        line: 3,
                                        col: 3,
                                        offset: 24,
                                    },
                                    rendered: "Neutrons",
                                },
                                source: Span {
                                    data: "Neutrons",
                                    line: 3,
                                    col: 3,
                                    offset: 24,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Neutrons",
                                line: 3,
                                col: 1,
                                offset: 22,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: ". Protons\n. Electrons\n. Neutrons",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ". Protons\n. Electrons\n. Neutrons",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    #[test]
    fn explicit_numbering() {
        verifies!(
            r#"
If you number the ordered list explicitly, you have to manually keep the list numerals sequential.
Otherwise, you will get a warning.
This differs from other lightweight markup languages.
But there's a reason for it.

Using explicit numbering is one way to adjust the numbering offset of a list (only supported in Asciidoctor 2.1.0 or better).
For instance, you can type:

----
include::example$ordered.adoc[tag=base-num-start]
----

"#
        );

        let doc = Parser::default().parse("4. Step four\n5. Step five\n6. Step six");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::ArabicNumeral(Span {
                                data: "4.",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Step four",
                                        line: 1,
                                        col: 4,
                                        offset: 3,
                                    },
                                    rendered: "Step four",
                                },
                                source: Span {
                                    data: "Step four",
                                    line: 1,
                                    col: 4,
                                    offset: 3,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "4. Step four",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::ArabicNumeral(Span {
                                data: "5.",
                                line: 2,
                                col: 1,
                                offset: 13,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Step five",
                                        line: 2,
                                        col: 4,
                                        offset: 16,
                                    },
                                    rendered: "Step five",
                                },
                                source: Span {
                                    data: "Step five",
                                    line: 2,
                                    col: 4,
                                    offset: 16,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "5. Step five",
                                line: 2,
                                col: 1,
                                offset: 13,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::ArabicNumeral(Span {
                                data: "6.",
                                line: 3,
                                col: 1,
                                offset: 26,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Step six",
                                        line: 3,
                                        col: 4,
                                        offset: 29,
                                    },
                                    rendered: "Step six",
                                },
                                source: Span {
                                    data: "Step six",
                                    line: 3,
                                    col: 4,
                                    offset: 29,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "6. Step six",
                                line: 3,
                                col: 1,
                                offset: 26,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "4. Step four\n5. Step five\n6. Step six",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "4. Step four\n5. Step five\n6. Step six",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    #[test]
    fn start_attribute() {
        verifies!(
            r#"
However, there's a simpler way to accomplish the same result without the manual effort.
You can use the `start` attribute on the list to define the number at which you want the numerals to start.

----
include::example$ordered.adoc[tag=base-start]
----

The start value is always a positive integer value, even when using a different numeration style such as loweralpha.

"#
        );

        let doc = Parser::default().parse("[start=4]\n. Step four\n. Step five\n. Step six");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 2,
                                col: 1,
                                offset: 10,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Step four",
                                        line: 2,
                                        col: 3,
                                        offset: 12,
                                    },
                                    rendered: "Step four",
                                },
                                source: Span {
                                    data: "Step four",
                                    line: 2,
                                    col: 3,
                                    offset: 12,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Step four",
                                line: 2,
                                col: 1,
                                offset: 10,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 3,
                                col: 1,
                                offset: 22,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Step five",
                                        line: 3,
                                        col: 3,
                                        offset: 24,
                                    },
                                    rendered: "Step five",
                                },
                                source: Span {
                                    data: "Step five",
                                    line: 3,
                                    col: 3,
                                    offset: 24,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Step five",
                                line: 3,
                                col: 1,
                                offset: 22,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 4,
                                col: 1,
                                offset: 34,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Step six",
                                        line: 4,
                                        col: 3,
                                        offset: 36,
                                    },
                                    rendered: "Step six",
                                },
                                source: Span {
                                    data: "Step six",
                                    line: 4,
                                    col: 3,
                                    offset: 36,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Step six",
                                line: 4,
                                col: 1,
                                offset: 34,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "[start=4]\n. Step four\n. Step five\n. Step six",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: Some("start",),
                            value: "4",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "start=4",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[start=4]\n. Step four\n. Step five\n. Step six",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    non_normative!(
        r#"
.When not to use the start attribute
****
When an ordered list item contains block content--such as an image, source block, or table--you may observe that the number of the next item in the list resets to 1.
In fact, what's happened is that a new list has been started where the number resets due to a missing list continuation.

In these cases, you should not resort to using the `start` attribute to fix the numbering.
Not only does that require manual adjustment as items are added to the list, it doesn't address the underlying semantics problem, which is what is causing it to be broken.
Instead, use a list continuation between each block element you want to attach to the list item to ensure the list item is continuous.
The list continuation glues the blocks together within a given item and keeps them at the same level of indentation.

* For details on how to use a list continuation, refer to the xref:continuation.adoc[] page.
* For an example of the list continuation used in an ordered list, see the launch steps in https://github.com/aws-quickstart/quickstart-microsoft-sql-fci-fsx/blob/main/docs/partner_editable/deploy_steps.adoc[this .adoc file in GitHub^].
* To see how those launch steps look in the final output, see the https://aws-quickstart.github.io/quickstart-microsoft-sql-fci-fsx/#_launch_the_quick_start[Launch the Quick Start^] section of the generated deployment guide.
The list continuations prevent step 2 from resetting to 1.
They also prevent step 5, which is pulled in from a separate AsciiDoc file, from resetting to 1.
****

"#
    );

    #[test]
    fn reversed_option() {
        verifies!(
            r#"
To present list items in reverse order, add the `reversed` option:

----
include::example$ordered.adoc[tag=reversed]
----

====
include::example$ordered.adoc[tag=reversed]
====

"#
        );

        let doc = Parser::default()
            .parse("[%reversed]\n.Parts of an atom\n. Protons\n. Electrons\n. Neutrons");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 3,
                                col: 1,
                                offset: 30,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Protons",
                                        line: 3,
                                        col: 3,
                                        offset: 32,
                                    },
                                    rendered: "Protons",
                                },
                                source: Span {
                                    data: "Protons",
                                    line: 3,
                                    col: 3,
                                    offset: 32,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Protons",
                                line: 3,
                                col: 1,
                                offset: 30,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 4,
                                col: 1,
                                offset: 40,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Electrons",
                                        line: 4,
                                        col: 3,
                                        offset: 42,
                                    },
                                    rendered: "Electrons",
                                },
                                source: Span {
                                    data: "Electrons",
                                    line: 4,
                                    col: 3,
                                    offset: 42,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Electrons",
                                line: 4,
                                col: 1,
                                offset: 40,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 5,
                                col: 1,
                                offset: 52,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Neutrons",
                                        line: 5,
                                        col: 3,
                                        offset: 54,
                                    },
                                    rendered: "Neutrons",
                                },
                                source: Span {
                                    data: "Neutrons",
                                    line: 5,
                                    col: 3,
                                    offset: 54,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Neutrons",
                                line: 5,
                                col: 1,
                                offset: 52,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "[%reversed]\n.Parts of an atom\n. Protons\n. Electrons\n. Neutrons",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: Some(Span {
                        data: "Parts of an atom",
                        line: 2,
                        col: 2,
                        offset: 13,
                    },),
                    title: Some("Parts of an atom",),
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "%reversed",
                            shorthand_items: &["%reversed"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "%reversed",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[%reversed]\n.Parts of an atom\n. Protons\n. Electrons\n. Neutrons",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    #[test]
    fn title() {
        verifies!(
            r#"
You can give a list a title by prefixing the line with a dot immediately followed by the text (without leaving any space after the dot).

Here's an example of a list with a title:

----
include::example$ordered.adoc[tag=base-t]
----

====
include::example$ordered.adoc[tag=base-t]
====

"#
        );

        let doc = Parser::default().parse(".Parts of an atom\n. Protons\n. Electrons\n. Neutrons");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 2,
                                col: 1,
                                offset: 18,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Protons",
                                        line: 2,
                                        col: 3,
                                        offset: 20,
                                    },
                                    rendered: "Protons",
                                },
                                source: Span {
                                    data: "Protons",
                                    line: 2,
                                    col: 3,
                                    offset: 20,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Protons",
                                line: 2,
                                col: 1,
                                offset: 18,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 3,
                                col: 1,
                                offset: 28,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Electrons",
                                        line: 3,
                                        col: 3,
                                        offset: 30,
                                    },
                                    rendered: "Electrons",
                                },
                                source: Span {
                                    data: "Electrons",
                                    line: 3,
                                    col: 3,
                                    offset: 30,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Electrons",
                                line: 3,
                                col: 1,
                                offset: 28,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 4,
                                col: 1,
                                offset: 40,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Neutrons",
                                        line: 4,
                                        col: 3,
                                        offset: 42,
                                    },
                                    rendered: "Neutrons",
                                },
                                source: Span {
                                    data: "Neutrons",
                                    line: 4,
                                    col: 3,
                                    offset: 42,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Neutrons",
                                line: 4,
                                col: 1,
                                offset: 40,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: ".Parts of an atom\n. Protons\n. Electrons\n. Neutrons",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: Some(Span {
                        data: "Parts of an atom",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },),
                    title: Some("Parts of an atom",),
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ".Parts of an atom\n. Protons\n. Electrons\n. Neutrons",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }
}

#[rustfmt::skip] // Used because this mod has deeply-nested data structures that cause the formatter to slow down significantly.
mod nested {
    use crate::{
        blocks::{ListType},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Nested ordered list

"#
    );

    #[test]
    fn basic_nested() {
        verifies!(
            r#"
// tag::basic[]
You create a nested item by using one or more dots in front of each the item.

----
include::example$ordered.adoc[tag=nest]
----

AsciiDoc selects a different number scheme for each level of nesting.
Here's how the previous list renders:

.A nested ordered list
====
include::example$ordered.adoc[tag=nest]
====
// end::basic[]

"#
        );

        let doc = Parser::default().parse(". Step 1\n. Step 2\n.. Step 2a\n.. Step 2b\n. Step 3");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Step 1",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "Step 1",
                                },
                                source: Span {
                                    data: "Step 1",
                                    line: 1,
                                    col: 3,
                                    offset: 2,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Step 1",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 2,
                                col: 1,
                                offset: 9,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Step 2",
                                            line: 2,
                                            col: 3,
                                            offset: 11,
                                        },
                                        rendered: "Step 2",
                                    },
                                    source: Span {
                                        data: "Step 2",
                                        line: 2,
                                        col: 3,
                                        offset: 11,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::List(ListBlock {
                                    type_: ListType::Ordered,
                                    items: &[
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Dots(Span {
                                                data: "..",
                                                line: 3,
                                                col: 1,
                                                offset: 18,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Step 2a",
                                                        line: 3,
                                                        col: 4,
                                                        offset: 21,
                                                    },
                                                    rendered: "Step 2a",
                                                },
                                                source: Span {
                                                    data: "Step 2a",
                                                    line: 3,
                                                    col: 4,
                                                    offset: 21,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: ".. Step 2a",
                                                line: 3,
                                                col: 1,
                                                offset: 18,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Dots(Span {
                                                data: "..",
                                                line: 4,
                                                col: 1,
                                                offset: 29,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Step 2b",
                                                        line: 4,
                                                        col: 4,
                                                        offset: 32,
                                                    },
                                                    rendered: "Step 2b",
                                                },
                                                source: Span {
                                                    data: "Step 2b",
                                                    line: 4,
                                                    col: 4,
                                                    offset: 32,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: ".. Step 2b",
                                                line: 4,
                                                col: 1,
                                                offset: 29,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                    ],
                                    source: Span {
                                        data: ".. Step 2a\n.. Step 2b",
                                        line: 3,
                                        col: 1,
                                        offset: 18,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: ". Step 2\n.. Step 2a\n.. Step 2b",
                                line: 2,
                                col: 1,
                                offset: 9,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 5,
                                col: 1,
                                offset: 40,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Step 3",
                                        line: 5,
                                        col: 3,
                                        offset: 42,
                                    },
                                    rendered: "Step 3",
                                },
                                source: Span {
                                    data: "Step 3",
                                    line: 5,
                                    col: 3,
                                    offset: 42,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Step 3",
                                line: 5,
                                col: 1,
                                offset: 40,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: ". Step 1\n. Step 2\n.. Step 2a\n.. Step 2b\n. Step 3",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ". Step 1\n. Step 2\n.. Step 2a\n.. Step 2b\n. Step 3",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    non_normative!(
        r#"
[TIP]
====
Like with the asterisks in an unordered list, the number of dots in an ordered list doesn't represent the nesting level.
However, it's much more intuitive to follow the convention that the number of dots equals the level of nesting.

*# of dots = level of nesting*

Again, we are shooting for plain text markup that is readable _as is_.
====

"#
    );

    #[test]
    fn mix_and_match() {
        verifies!(
            r#"
You can mix and match the three list types, ordered, xref:unordered.adoc[unordered], and xref:description.adoc[description], within a single hybrid list.
The AsciiDoc syntax tries hard to infer the relationships between the items that are most intuitive to us humans.

Here's an example of nesting an unordered list inside of an ordered list:

----
include::example$ordered.adoc[tag=mix]
----

====
include::example$ordered.adoc[tag=mix]
====

"#
        );

        let doc = Parser::default()
            .parse(". Linux\n* Fedora\n* Ubuntu\n* Slackware\n. BSD\n* FreeBSD\n* NetBSD");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Linux",
                                            line: 1,
                                            col: 3,
                                            offset: 2,
                                        },
                                        rendered: "Linux",
                                    },
                                    source: Span {
                                        data: "Linux",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::List(ListBlock {
                                    type_: ListType::Unordered,
                                    items: &[
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 2,
                                                col: 1,
                                                offset: 8,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Fedora",
                                                        line: 2,
                                                        col: 3,
                                                        offset: 10,
                                                    },
                                                    rendered: "Fedora",
                                                },
                                                source: Span {
                                                    data: "Fedora",
                                                    line: 2,
                                                    col: 3,
                                                    offset: 10,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "* Fedora",
                                                line: 2,
                                                col: 1,
                                                offset: 8,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 3,
                                                col: 1,
                                                offset: 17,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Ubuntu",
                                                        line: 3,
                                                        col: 3,
                                                        offset: 19,
                                                    },
                                                    rendered: "Ubuntu",
                                                },
                                                source: Span {
                                                    data: "Ubuntu",
                                                    line: 3,
                                                    col: 3,
                                                    offset: 19,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "* Ubuntu",
                                                line: 3,
                                                col: 1,
                                                offset: 17,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 4,
                                                col: 1,
                                                offset: 26,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Slackware",
                                                        line: 4,
                                                        col: 3,
                                                        offset: 28,
                                                    },
                                                    rendered: "Slackware",
                                                },
                                                source: Span {
                                                    data: "Slackware",
                                                    line: 4,
                                                    col: 3,
                                                    offset: 28,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "* Slackware",
                                                line: 4,
                                                col: 1,
                                                offset: 26,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                    ],
                                    source: Span {
                                        data: "* Fedora\n* Ubuntu\n* Slackware",
                                        line: 2,
                                        col: 1,
                                        offset: 8,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: ". Linux\n* Fedora\n* Ubuntu\n* Slackware",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 5,
                                col: 1,
                                offset: 38,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "BSD",
                                            line: 5,
                                            col: 3,
                                            offset: 40,
                                        },
                                        rendered: "BSD",
                                    },
                                    source: Span {
                                        data: "BSD",
                                        line: 5,
                                        col: 3,
                                        offset: 40,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::List(ListBlock {
                                    type_: ListType::Unordered,
                                    items: &[
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 6,
                                                col: 1,
                                                offset: 44,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "FreeBSD",
                                                        line: 6,
                                                        col: 3,
                                                        offset: 46,
                                                    },
                                                    rendered: "FreeBSD",
                                                },
                                                source: Span {
                                                    data: "FreeBSD",
                                                    line: 6,
                                                    col: 3,
                                                    offset: 46,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "* FreeBSD",
                                                line: 6,
                                                col: 1,
                                                offset: 44,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 7,
                                                col: 1,
                                                offset: 54,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "NetBSD",
                                                        line: 7,
                                                        col: 3,
                                                        offset: 56,
                                                    },
                                                    rendered: "NetBSD",
                                                },
                                                source: Span {
                                                    data: "NetBSD",
                                                    line: 7,
                                                    col: 3,
                                                    offset: 56,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "* NetBSD",
                                                line: 7,
                                                col: 1,
                                                offset: 54,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                    ],
                                    source: Span {
                                        data: "* FreeBSD\n* NetBSD",
                                        line: 6,
                                        col: 1,
                                        offset: 44,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: ". BSD\n* FreeBSD\n* NetBSD",
                                line: 5,
                                col: 1,
                                offset: 38,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: ". Linux\n* Fedora\n* Ubuntu\n* Slackware\n. BSD\n* FreeBSD\n* NetBSD",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ". Linux\n* Fedora\n* Ubuntu\n* Slackware\n. BSD\n* FreeBSD\n* NetBSD",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    #[test]
    fn mix_alt() {
        verifies!(
            r#"
You can spread the items out and indent the nested lists if that makes it more readable for you:

----
include::example$ordered.adoc[tag=mix-alt]
----

The description list page demonstrates how to xref:description.adoc#three-hybrid[combine all three list types].

"#
        );

        let doc = Parser::default()
            .parse(". Linux\n\n  * Fedora\n  * Ubuntu\n  * Slackware\n\n. BSD\n\n  * FreeBSD\n  * NetBSD\n");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Linux",
                                            line: 1,
                                            col: 3,
                                            offset: 2,
                                        },
                                        rendered: "Linux",
                                    },
                                    source: Span {
                                        data: "Linux",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::List(ListBlock {
                                    type_: ListType::Unordered,
                                    items: &[
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 3,
                                                col: 3,
                                                offset: 11,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Fedora",
                                                        line: 3,
                                                        col: 5,
                                                        offset: 13,
                                                    },
                                                    rendered: "Fedora",
                                                },
                                                source: Span {
                                                    data: "Fedora",
                                                    line: 3,
                                                    col: 5,
                                                    offset: 13,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "  * Fedora",
                                                line: 3,
                                                col: 1,
                                                offset: 9,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 4,
                                                col: 3,
                                                offset: 22,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Ubuntu",
                                                        line: 4,
                                                        col: 5,
                                                        offset: 24,
                                                    },
                                                    rendered: "Ubuntu",
                                                },
                                                source: Span {
                                                    data: "Ubuntu",
                                                    line: 4,
                                                    col: 5,
                                                    offset: 24,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "  * Ubuntu",
                                                line: 4,
                                                col: 1,
                                                offset: 20,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 5,
                                                col: 3,
                                                offset: 33,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Slackware",
                                                        line: 5,
                                                        col: 5,
                                                        offset: 35,
                                                    },
                                                    rendered: "Slackware",
                                                },
                                                source: Span {
                                                    data: "Slackware",
                                                    line: 5,
                                                    col: 5,
                                                    offset: 35,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "  * Slackware",
                                                line: 5,
                                                col: 1,
                                                offset: 31,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                    ],
                                    source: Span {
                                        data: "  * Fedora\n  * Ubuntu\n  * Slackware",
                                        line: 3,
                                        col: 1,
                                        offset: 9,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: ". Linux\n\n  * Fedora\n  * Ubuntu\n  * Slackware",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 7,
                                col: 1,
                                offset: 46,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "BSD",
                                            line: 7,
                                            col: 3,
                                            offset: 48,
                                        },
                                        rendered: "BSD",
                                    },
                                    source: Span {
                                        data: "BSD",
                                        line: 7,
                                        col: 3,
                                        offset: 48,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::List(ListBlock {
                                    type_: ListType::Unordered,
                                    items: &[
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 9,
                                                col: 3,
                                                offset: 55,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "FreeBSD",
                                                        line: 9,
                                                        col: 5,
                                                        offset: 57,
                                                    },
                                                    rendered: "FreeBSD",
                                                },
                                                source: Span {
                                                    data: "FreeBSD",
                                                    line: 9,
                                                    col: 5,
                                                    offset: 57,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "  * FreeBSD",
                                                line: 9,
                                                col: 1,
                                                offset: 53,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "*",
                                                line: 10,
                                                col: 3,
                                                offset: 67,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "NetBSD",
                                                        line: 10,
                                                        col: 5,
                                                        offset: 69,
                                                    },
                                                    rendered: "NetBSD",
                                                },
                                                source: Span {
                                                    data: "NetBSD",
                                                    line: 10,
                                                    col: 5,
                                                    offset: 69,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "  * NetBSD",
                                                line: 10,
                                                col: 1,
                                                offset: 65,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                    ],
                                    source: Span {
                                        data: "  * FreeBSD\n  * NetBSD",
                                        line: 9,
                                        col: 1,
                                        offset: 53,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: ". BSD\n\n  * FreeBSD\n  * NetBSD",
                                line: 7,
                                col: 1,
                                offset: 46,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: ". Linux\n\n  * Fedora\n  * Ubuntu\n  * Slackware\n\n. BSD\n\n  * FreeBSD\n  * NetBSD",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ". Linux\n\n  * Fedora\n  * Ubuntu\n  * Slackware\n\n. BSD\n\n  * FreeBSD\n  * NetBSD",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }
}

// The number styles section is considered non-normative because the parser
// crate does not specify rendering or CSS styles.
non_normative!(
    r#"
[#styles]
== Number styles

For ordered lists, AsciiDoc supports the numeration styles such as lowergreek and decimal-leading-zero.
The full list of numeration styles that can be applied to an ordered list are as follows:

[%autowidth]
|===
|Block style |CSS list-style-type

|arabic
|decimal

|decimal ^[1]^
|decimal-leading-zero

|loweralpha
|lower-alpha

|upperalpha
|upper-alpha

|lowerroman
|lower-roman

|upperroman
|upper-roman

|lowergreek ^[1]^
|lower-greek
|===
^[1]^ These styles are only supported by the HTML converters.

Here are a few examples showing various numeration styles as defined by the block style shown in the header row:

[%autowidth]
|===
|[arabic] ^[2]^ |[decimal] |[loweralpha] |[lowergreek]

a|
. one
. two
. three

a|
[decimal]
. one
. two
. three

a|
[loweralpha]
. one
. two
. three

a|
[lowergreek]
. one
. two
. three
|===
^[2]^ Default numeration if block style is not specified

TIP: Custom numeration styles can be implemented using a custom role.
Define a new class selector (e.g., `.custom`) in your stylesheet that sets the `list-style-type` property to the value of your choice.
Then, assign the name of that class as a role on any list to which you want that numeration applied.

When the role shorthand (`.custom`) is used on an ordered list, the numeration style is no longer omitted.

You can override the number scheme for any level by setting its style (the first positional entry in a block attribute list).
You can also set the starting number using the `start` attribute:

----
include::example$ordered.adoc[tag=num]
----

====
include::example$ordered.adoc[tag=num]
====

IMPORTANT: The `start` attribute must be a number, even when using a different numeration style.
For instance, to start an alphabetic list at letter "c", set the numeration style to loweralpha and the start attribute to 3.

"#
);

mod escaping {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
== Escaping the list marker

"#
    );
    #[test]
    fn mix_alt() {
        verifies!(
            r#"
If you have paragraph text that begins with a list marker, but you don't intend it to be a list item, you need to escape that marker by using the attribute reference to disrupt the pattern.

Consider the case when the line starts with a P.O. box reference:

----
P. O. Box
----

In order to prevent this paragraph from being parsed as an ordered list, you need to replace the first space with `\{empty}`.

----
P.{empty}O. Box
----

Now the paragraph will remain as a paragraph.

"#
        );

        let doc = Parser::default().parse("P.{empty}O. Box");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "P.{empty}O. Box",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "P.O. Box",
                    },
                    source: Span {
                        data: "P.{empty}O. Box",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "P.{empty}O. Box",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    non_normative!(
        r#"
In the future, it will be possible to escape an ordered list marker using a backslash, but that is not currently possible.
"#
    );
}
