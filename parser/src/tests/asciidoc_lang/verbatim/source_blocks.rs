use crate::{blocks::ContentModel, content::SubstitutionGroup, tests::prelude::*};

track_file!("docs/modules/verbatim/pages/source-blocks.adoc");

non_normative!(
    r##"
= Source Code Blocks
:table-caption: Table
:url-coderay: http://coderay.rubychan.de/
:url-highlightjs: https://highlightjs.org/
:url-pygments: https://pygments.org
:url-rouge: http://rouge.jneen.net
////
From user manual:
NOTE we really want this ID on the section, but there are problems w/ multiple IDs per section
[#source-code-blocks]
[#syntax-highlighting]
////

A source block is a specialization of a xref:listing-blocks.adoc[listing block].
Developers are accustomed to seeing source code colorized to emphasize the code's structure (i.e., keywords, types, delimiters, etc.).
This technique is known as [.term]*syntax highlighting*.
Since this technique is so prevalent, AsciiDoc processors will integrate at least one library to syntax highlight the source code blocks in your document.
For example, Asciidoctor provides integration with Rouge, CodeRay, Pygments, and highlight.js, as well as an adapter API to add support for additional libraries.

"##
);

#[test]
fn ex_source() {
    verifies!(
        r##"
<<ex-source>> shows a listing block with the `source` style and language `ruby` applied to its content, hence a source block.

.Source block syntax
[source#ex-source]
....
include::example$source.adoc[tag=src-base]
....

The result of <<ex-source>> is rendered below.

include::example$source.adoc[tag=src-base]

"##
    );

    let doc = Parser::default().parse(
        "[source,ruby]\n----\nrequire 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend\n----\n",
    );

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
            blocks: &[Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "require 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend",
                        line: 3,
                        col: 1,
                        offset: 19,
                    },
                    rendered: "require 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend",
                },
                content_model: ContentModel::Verbatim,
                context: "listing",
                source: Span {
                    data: "[source,ruby]\n----\nrequire 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend\n----",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            value: "source",
                            shorthand_items: &["source"],
                        },
                        ElementAttribute {
                            name: None,
                            value: "ruby",
                            shorthand_items: &[],
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "source,ruby",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
                substitution_group: SubstitutionGroup::Verbatim,
            },),],
            source: Span {
                data: "[source,ruby]\n----\nrequire 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend\n----",
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
fn ex_implied_source() {
    verifies!(
        r##"
Since a `source` block is most often used to designate a block with source code of a particular language, the `source` style itself is optional.
The mere presence of the language on a listing block automatically promotes it to a source block.

<<ex-implied-source>> shows a listing block implied to be a source block because a language is specified.

.Implied source block
[source#ex-implied-source]
....
include::example$source.adoc[tag=src-implied]
....

"##
    );

    let doc = Parser::default()
        .parse("[,ruby]\n----\nrequire 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend\n----");

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
            blocks: &[Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "require 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend",
                        line: 3,
                        col: 1,
                        offset: 13,
                    },
                    rendered: "require 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend",
                },
                content_model: ContentModel::Verbatim,
                context: "listing",
                source: Span {
                    data: "[,ruby]\n----\nrequire 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend\n----",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            value: "",
                            shorthand_items: &[],
                        },
                        ElementAttribute {
                            name: None,
                            value: "ruby",
                            shorthand_items: &[],
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: ",ruby",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
                substitution_group: SubstitutionGroup::Verbatim,
            },),],
            source: Span {
                data: "[,ruby]\n----\nrequire 'sinatra'\n\nget '/hi' do\n  \"Hello World!\"\nend\n----",
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
    r##"
This shorthand also works if the `source-language` attribute is set on the document, which serves as the default language for source blocks.
If the `source-language` attribute is set on the document and you want to make a regular listing block, add the `listing` style to the block.

== Using include directives in source blocks

"##
);

#[test]
fn ex_include() {
    verifies!(
        r##"
You can use an xref:directives:include.adoc[include directive] to insert source code into an AsciiDoc document directly from a file.

.Code inserted from another file
[listing#ex-include]
....
include::example$source.adoc[tag=src-inc]
....

"##
    );

    let doc = Parser::default().parse("[,ruby]\n----\ninclude::app.rb[]\n----");

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
            blocks: &[Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "Unresolved directive in (root file) - include::app.rb[]",
                        line: 3,
                        col: 1,
                        offset: 13,
                    },
                    rendered: "Unresolved directive in (root file) - include::app.rb[]",
                },
                content_model: ContentModel::Verbatim,
                context: "listing",
                source: Span {
                    data: "[,ruby]\n----\nUnresolved directive in (root file) - include::app.rb[]\n----",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            value: "",
                            shorthand_items: &[],
                        },
                        ElementAttribute {
                            name: None,
                            value: "ruby",
                            shorthand_items: &[],
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: ",ruby",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
                substitution_group: SubstitutionGroup::Verbatim,
            },),],
            source: Span {
                data: "[,ruby]\n----\nUnresolved directive in (root file) - include::app.rb[]\n----",
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
    r##"
//TODO mention the use of AsciiDoc tags to include code snippets and the indent flag to reset indentation

TIP: If you specify custom substitutions on the source block using the `subs` attribute, make sure to include the `specialcharacters` substitution if you want to preserve syntax highlighting.
However, if you do plan to modify the substitutions, we recommend using xref:subs:apply-subs-to-blocks.adoc#incremental[incremental substitutions] instead.

// Highlight PHP sidebar was taken from end of page and made its own page
"##
);
