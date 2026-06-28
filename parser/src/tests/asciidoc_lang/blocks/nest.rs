use crate::tests::prelude::*;

track_file!("docs/modules/blocks/pages/nest.adoc");

non_normative!(
    r#"
= Nesting Blocks
:page-archived:

Moved xref:delimited.adoc#nesting[here].
"#
);
