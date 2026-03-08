use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/icon-macro.adoc");

non_normative!(
    r#"
= Icon Macro

In addition to built-in icons, you can add icons anywhere in your content where macros are substituted using the icon macro.
This page covers the anatomy of the icon macro, how the target is resolved, and what features it support (subject to the icon mode).

"#
);

mod anatomy {
    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Anatomy

The icon macro is an inline macro.
Like other inline macros, its syntax follows the familiar pattern of the macro name and target separated by a colon followed by an attribute list enclosed in square brackets.

[source]
----
icon:<target>[<attrlist>]
----

The `<target>` is the icon name or path.
The `<attrlist>` specifies various named attributes to configure how the icon is displayed.

"#
    );

    #[test]
    fn example() {
        verifies!(
            r#"
For example:

[source]
----
icon:heart[2x,role=red]
----

"#
        );

        let doc = Parser::default().parse("icon:heart[2x,role=red]");

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
                            data: "icon:heart[2x,role=red]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<span class=\"icon red\">[heart&#93;</span>",
                    },
                    source: Span {
                        data: "icon:heart[2x,role=red]",
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
                    data: "icon:heart[2x,role=red]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }
}
