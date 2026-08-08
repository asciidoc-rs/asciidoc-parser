//! A first-class **inline AST**: the structured representation of the inline
//! content of a leaf block.
//!
//! This module defines the public node vocabulary described in the [inline AST
//! architecture] design document. It is aligned with the Eclipse AsciiDoc
//! Language project's [Abstract Semantic Graph (ASG)]: the ASG's small inline
//! core – `span` / `ref` / `text` / `charref` / `raw` – forms the spine, and
//! the constructs this crate supports beyond that core (images, footnotes, UI
//! macros, index terms, callouts, anchors, line breaks, and STEM) are modeled
//! as additional variants that project down to ASG-legal nodes when emitting
//! conformant ASG.
//!
//! # Status: skeleton (Phase 0)
//!
//! These are **types only, with no wiring.** Nothing in the parser or the
//! rendering pipeline produces or consumes them yet; [`Content`] still exposes
//! inline content as a rendered string. The types are landed first so the
//! public shape can be reviewed and the later phases – building the tree,
//! making it canonical, and folding it back to HTML – can proceed against a
//! stable vocabulary. Field sets on the crate-extension nodes are provisional
//! and will be refined against the first real consumer.
//!
//! # Logical text, not output text
//!
//! A node holds the **reader's** characters, not escaped HTML. HTML-escaping is
//! the renderer's job, performed when the tree is folded to output. This is the
//! meaning of the ASG's `text` / `charref` / `raw` trichotomy, captured here by
//! [`InlineNode::Text`], [`InlineNode::CharRef`], and [`InlineNode::Raw`].
//!
//! [inline AST architecture]: https://github.com/scouten/asciidoc-parser/blob/main/docs/design/inline-ast-architecture.md
//! [Abstract Semantic Graph (ASG)]: https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/main/asg/schema.json
//! [`Content`]: crate::content::Content

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
pub use inline_node::InlineNode;

mod ref_node;
pub use ref_node::{Ref, RefVariant};

mod stem;
pub use stem::{Stem, StemNotation};

mod styled;
pub use styled::{SpanForm, StyleVariant, Styled};

mod ui;
pub use ui::{Ui, UiKind};
