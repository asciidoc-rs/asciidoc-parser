use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/blocks/pages/styles.adoc");

non_normative!(
    r#"
= Block Style
:page-archived:

Moved xref:index.adoc#block-style[here].
"#
);
