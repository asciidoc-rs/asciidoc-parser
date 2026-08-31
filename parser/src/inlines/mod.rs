//! A first-class **inline AST**: the structured representation of the inline
//! content of a leaf block.
//!
//! This module defines the public node vocabulary. It is aligned with the
//! Eclipse AsciiDoc Language project's [Abstract Semantic Graph (ASG)]: the
//! ASG's small inline core — `span` / `ref` / `text` / `charref` / `raw` —
//! forms the spine, and the constructs this crate supports beyond that core
//! (images, footnotes, UI macros, index terms, callouts, anchors, line
//! breaks, and STEM) are modeled as additional variants that project down to
//! ASG-legal nodes when emitting conformant ASG.
//!
//! This tree is the crate's single inline representation, built directly from
//! source; both [`Content::rendered_html`] and every macro family's
//! catalog/warning registration are derived from it.
//!
//! # Logical text, not output text
//!
//! A node holds the **reader's** characters, not escaped HTML. HTML-escaping is
//! the renderer's job, performed when the tree is folded to output. This is the
//! meaning of the ASG's `text` / `charref` / `raw` trichotomy, captured here by
//! [`InlineNode::Text`], [`InlineNode::CharRef`], and [`InlineNode::Raw`].
//!
//! [Abstract Semantic Graph (ASG)]: https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/main/asg/schema.json
//! [`Content::rendered_html`]: crate::content::Content::rendered_html

mod anchor;
pub use anchor::Anchor;

mod callout;
pub use callout::{Callout, CalloutGuard};

mod char_ref;
pub use char_ref::CharRef;

mod footnote;
pub use footnote::Footnote;

mod image;
pub use image::Image;

mod index_term;
pub use index_term::IndexTerm;

mod inline_node;
pub use inline_node::{InlineNode, RawForm, RawOrigin};

mod ref_node;
pub use ref_node::{LinkForm, Ref, RefVariant};

mod stem;
pub use stem::{Stem, StemNotation};

mod styled;
pub use styled::{PassthroughWrapper, SpanForm, StyleVariant, Styled};

mod ui;
pub use ui::{Ui, UiKind};
