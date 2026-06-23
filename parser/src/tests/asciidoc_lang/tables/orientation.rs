use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/orientation.adoc");

// This entire page is considered non-normative because table orientation
// (the `rotate` option and `orientation` attribute) only affects rendering, and
// as the page itself notes, it is implemented solely by the DocBook backend.
// This crate doesn't do rendering.
non_normative!(
    r#"
= Table Orientation

== Landscape

A table can be displayed in landscape (rotated 90 degrees counterclockwise) using the `rotate` option (preferred).

[source]
----
[%rotate]
include::example$table.adoc[tag=base]
----

A table can also be displayed in landscape using the `orientation` attribute.

[source]
----
[orientation=landscape]
include::example$table.adoc[tag=base]
----

Currently, this is only implemented by the DocBook backend (it sets the attribute `orient="land"`).
"#
);
