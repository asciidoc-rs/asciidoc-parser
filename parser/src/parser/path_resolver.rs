use std::sync::LazyLock;

use regex::Regex;

/// A `PathResolver` handles all operations for resolving, cleaning, and joining
/// paths. This struct includes operations for handling both web paths (request
/// URIs) and system paths.
///
/// The main emphasis of the struct is on creating clean and secure paths. Clean
/// paths are void of duplicate parent and current directory references in the
/// path name. Secure paths are paths which are restricted from accessing
/// directories outside of a jail path, if specified.
///
/// Since joining two paths can result in an insecure path, this struct also
/// handles the task of joining a parent (start) and child (target) path.
///
/// Like its counterpart in the Ruby Asciidoctor implementation, this struct
/// makes no use of path utilities from the underlying Rust libraries. Instead,
/// it handles all aspects of path manipulation. The main benefit of
/// internalizing these operations is that the struct is able to handle both
/// Posix and Windows paths independent of the operating system on which it
/// runs. This makes the class both deterministic and easier to test.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathResolver {
    /// File separator to use for path operations. (Defaults to
    /// platform-appropriate separator.)
    pub file_separator: char,
    // Ruby's `PathResolver` also carries a `working_dir`, but it is read only by
    // `system_path` — the filesystem-resolution + safe-mode-jail routine — as a
    // fallback base directory, and Ruby always seeds it from `Dir.pwd`. This
    // crate performs no filesystem I/O: `include::` and asset resolution are
    // delegated to the client via `IncludeFileHandler`, so there is no consumer
    // for `working_dir`, and a `Dir.pwd`-derived base would contradict this
    // type's deterministic, OS-independent design. Intentionally not ported; if
    // a built-in filesystem handler ever ports `system_path`, prefer an explicit
    // caller-supplied base over an implicit working directory.
}

impl Default for PathResolver {
    fn default() -> Self {
        Self {
            file_separator: std::path::MAIN_SEPARATOR,
        }
    }
}

impl PathResolver {
    /// Normalize path by converting any backslashes to forward slashes.
    pub fn posixify(&self, path: &str) -> String {
        if self.file_separator == '\\' && path.contains('\\') {
            path.replace('\\', "/")
        } else {
            path.to_string()
        }
    }

    /// Resolve a web path from the target and start paths.
    ///
    /// The main function of this operation is to resolve any parent references
    /// and remove any self references.
    ///
    /// The target is assumed to be a path, not a qualified URI. That check
    /// should happen before this method is invoked.
    ///
    /// Returns a path that joins the target path with the start path with any
    /// parent references resolved and self references removed.
    pub fn web_path(&self, target: &str, start: Option<&str>) -> String {
        let mut target = self.posixify(target);
        let start = start.map(|start| self.posixify(start));

        let mut uri_prefix: Option<String> = None;

        // Ruby treats a `nil` *or* empty `start` the same (`start.nil_or_empty?`):
        // in both cases the target is used as-is, with no start path prepended.
        let start_is_empty = start.as_deref().is_none_or(str::is_empty);

        if !(start_is_empty || self.is_web_root(&target)) {
            (target, uri_prefix) = extract_uri_prefix(&format!(
                "{start}{maybe_add_slash}{target}",
                start = start.as_deref().unwrap_or_default(),
                maybe_add_slash = start
                    .as_ref()
                    .map(|s| if s.ends_with("/") { "" } else { "/" })
                    .unwrap_or_default()
            ));
        }

        let (target_segments, target_root) = self.partition_path(&target, WebPath(true));

        let mut resolved_segments: Vec<String> = vec![];

        for segment in target_segments {
            if segment == ".." {
                if resolved_segments.is_empty() {
                    if let Some(target_root) = target_root.as_ref()
                        && target_root != "./"
                    {
                        // Do nothing.
                    } else {
                        resolved_segments.push(segment);
                    }
                } else if let Some(last_segment) = resolved_segments.last()
                    && last_segment == ".."
                {
                    resolved_segments.push(segment);
                } else {
                    resolved_segments.pop();
                }
            } else {
                resolved_segments.push(segment);
            }
        }

        let resolved_path = self
            .join_path(&resolved_segments, target_root.as_deref())
            .replace(" ", "%20");

        format!(
            "{uri_prefix}{resolved_path}",
            uri_prefix = uri_prefix.unwrap_or_default()
        )
    }

    /// Partition the path into path segments and remove self references (`.`)
    /// and the trailing slash, if present. Prior to being partitioned, the path
    /// is converted to a Posix path.
    ///
    /// Parent references are not resolved by this method since the caller often
    /// needs to handle this resolution in a certain context (checking for the
    /// breach of a jail, for instance).
    ///
    /// Returns a 2-item tuple containing a `Vec<String>` of path segments and
    /// an optional path root (e.g., `/`, `./`, `c:/`, or `//`), which is only
    /// present if the path is absolute.
    fn partition_path(&self, path: &str, web: WebPath) -> (Vec<String>, Option<String>) {
        // Ruby memoizes partition results per (path, web) in a hash. That cache
        // exists to avoid Ruby's comparatively expensive string work; here the
        // partitioning is cheap and the resolver takes `&self`, so caching would
        // require interior mutability that the derived `Clone`/`Eq` would then
        // have to skip. Deliberately not ported.
        let posix_path = self.posixify(path);

        let root: Option<String> = if web.0 {
            if self.is_web_root(&posix_path) {
                Some("/".to_owned())
            } else if posix_path.starts_with("./") {
                Some("./".to_owned())
            } else {
                None
            }
        } else if self.is_root(&posix_path) {
            if self.is_unc(&posix_path) {
                // ex. //sample/path
                Some(DOUBLE_SLASH.to_owned())
            } else if posix_path.starts_with('/') {
                // ex. /sample/path
                Some("/".to_owned())
            } else if posix_path.starts_with(URI_CLASSLOADER) {
                // ex. uri:classloader:sample/path (or uri:classloader:/sample/path)
                Some(URI_CLASSLOADER.to_owned())
            } else {
                // ex. C:/sample/path (or file:///sample/path in browser
                // environment). The root is everything up to and including the
                // first slash. `is_root` guarantees a slash for the drive-letter
                // case, so `None` here is unreachable in practice.
                posix_path
                    .find('/')
                    .map(|slash| posix_path[..=slash].to_owned())
            }
        } else if posix_path.starts_with("./") {
            // ex. ./sample/path
            Some("./".to_owned())
        } else {
            // otherwise ex. sample/path
            None
        };

        let path_after_root = if let Some(root) = &root {
            &posix_path[root.len()..]
        } else {
            &posix_path
        };

        let mut path_segments: Vec<String> = path_after_root
            .split('/')
            .filter(|s| *s != ".")
            .map(|s| s.to_owned())
            .collect();

        // Ruby builds these segments with `posix_path.split SLASH`, and Ruby's
        // `String#split` drops trailing empty fields (e.g. `'a/b/'.split '/'` is
        // `['a', 'b']`, and `''.split '/'` is `[]`). Rust's `str::split` keeps
        // them, so strip trailing empties to match — otherwise a trailing slash
        // would survive as a spurious empty final segment.
        while path_segments.last().is_some_and(|s| s.is_empty()) {
            path_segments.pop();
        }

        (path_segments, root)
    }

    /// Join the segments using the Posix file separator (since this crate knows
    /// how to work with paths specified this way, regardless of OS). Use the
    /// `root`, if specified, to construct an absolute path. Otherwise join the
    /// segments as a relative path.
    fn join_path(&self, segments: &[String], root: Option<&str>) -> String {
        format!(
            "{root}{segments}",
            root = root.unwrap_or_default(),
            segments = segments.join("/"),
        )
    }

    /// Return `true` if the path is an absolute (root) web path (i.e. starts
    /// with a `'/'`.
    pub fn is_web_root(&self, path: &str) -> bool {
        path.starts_with('/')
    }

    /// Return `true` if the path is an absolute (rooted) system path. A path is
    /// absolute if it starts with `'/'` or, on a Windows-style resolver, if it
    /// starts with a drive letter or slash root (e.g. `C:/` or `\`).
    fn is_absolute_path(&self, path: &str) -> bool {
        path.starts_with('/') || (self.file_separator == '\\' && WINDOWS_ROOT.is_match(path))
    }

    /// Return `true` if the path has a root, meaning it is either an absolute
    /// path or a `uri:classloader:` path.
    fn is_root(&self, path: &str) -> bool {
        self.is_absolute_path(path) || path.starts_with(URI_CLASSLOADER)
    }

    /// Return `true` if the path is a UNC path (i.e. starts with `'//'`).
    fn is_unc(&self, path: &str) -> bool {
        path.starts_with(DOUBLE_SLASH)
    }
}

/// Efficiently extracts the URI prefix from the specified string if the string
/// is a URI.
///
/// Attempts to match the URI prefix in the specified string (e.g., `http://`). If present, the prefix is removed.
///
/// Returns a tuple containing the specified string without the URI prefix, if
/// present, and the extracted URI prefix if found.
fn extract_uri_prefix(s: &str) -> (String, Option<String>) {
    if s.contains(':')
        && let Some(prefix) = URI_SNIFF.find(s)
    {
        (
            s[prefix.len()..].to_string(),
            Some(prefix.as_str().to_owned()),
        )
    } else {
        (s.to_string(), None)
    }
}

// Also: Place this at module scope:
static URI_SNIFF: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)
        ^                   # Anchor: start of string

        \p{Alphabetic}      # First character: a Unicode letter

        [\p{Alphabetic}     # Followed by one or more of:
        \p{Number}         #   - Unicode letters or numbers
        .                  #   - Period
        \+                 #   - Plus sign
        \-                 #   - Hyphen
        ]+                  # One or more of the above

        :                   # Followed by a literal colon

        /{0,2}              # Followed by zero, one, or two literal slashes
    "#,
    )
    .unwrap()
});

/// Path root marker for a UNC path (e.g. `//sample/path`).
const DOUBLE_SLASH: &str = "//";

/// Path root marker for a JRuby classloader URI (e.g.
/// `uri:classloader:/sample/path`).
const URI_CLASSLOADER: &str = "uri:classloader:";

/// Matches the root of a Windows-style path: an optional drive letter followed
/// by a leading slash (either separator). Mirrors Ruby Asciidoctor's
/// `WindowsRootRx`.
static WINDOWS_ROOT: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"^(?:[a-zA-Z]:)?[\\/]").unwrap()
});

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WebPath(pub(crate) bool);

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::parser::PathResolver;

    mod posixify {
        use crate::parser::PathResolver;

        #[test]
        fn replaces_backslashes_if_windowsish() {
            let pr = PathResolver {
                file_separator: '\\',
            };

            assert_eq!(pr.posixify("abc/def\\ghi"), "abc/def/ghi");
        }

        #[test]
        fn doesnt_replace_backslashes_if_posixish() {
            let pr = PathResolver {
                file_separator: '/',
            };

            assert_eq!(pr.posixify("abc/def\\ghi"), "abc/def\\ghi");
        }

        #[test]
        fn doesnt_replace_backslashes_if_none_exist() {
            let pr = PathResolver {
                file_separator: '\\',
            };

            assert_eq!(pr.posixify("abc/def"), "abc/def");
        }
    }

    mod web_path {
        use crate::parser::PathResolver;

        #[test]
        fn test_cases_from_asciidoctor_rb() {
            let pr = PathResolver::default();

            assert_eq!(pr.web_path("images", None), "images");
            assert_eq!(pr.web_path("./images", None), "./images");
            assert_eq!(pr.web_path("/images", None), "/images");

            assert_eq!(
                pr.web_path("./images/../assets/images", None),
                "./assets/images"
            );

            assert_eq!(pr.web_path("/../images", None), "/images");

            assert_eq!(pr.web_path("/../images", Some("assets")), "/images");
            assert_eq!(pr.web_path("../images", Some("./")), "./../images");
            assert_eq!(pr.web_path("../../images", Some("./")), "./../../images");

            assert_eq!(
                pr.web_path("tiger.png", Some("../assets/images")),
                "../assets/images/tiger.png"
            );

            // Basic relative path resolution.
            assert_eq!(
                pr.web_path("images/photo.jpg", Some("docs/guide")),
                "docs/guide/images/photo.jpg"
            );
            assert_eq!(pr.web_path("photo.jpg", Some("images")), "images/photo.jpg");
            assert_eq!(
                pr.web_path("../photo.jpg", Some("images/folder")),
                "images/photo.jpg"
            );
            assert_eq!(
                pr.web_path("../../photo.jpg", Some("docs/images/folder")),
                "docs/photo.jpg"
            );

            // URI-based scenarios (triggers `extract_uri_prefix`).
            assert_eq!(
                pr.web_path("images/photo.jpg", Some("http://example.com/base")),
                "http://example.com/base/images/photo.jpg"
            );
            assert_eq!(
                pr.web_path("../images/logo.png", Some("https://cdn.example.com/assets")),
                "https://cdn.example.com/images/logo.png"
            );
            assert_eq!(
                pr.web_path("docs/guide.pdf", Some("file:///Users/docs")),
                "file:///Users/docs/docs/guide.pdf"
            );
            assert_eq!(
                pr.web_path("assets/style.css", Some("ftp://files.example.com/web")),
                "ftp://files.example.com/web/assets/style.css"
            );

            // Web root scenarios (start parameter ignored).
            assert_eq!(
                pr.web_path("/absolute/path.jpg", Some("http://example.com/base")),
                "/absolute/path.jpg"
            );
            assert_eq!(
                pr.web_path("/images/photo.jpg", Some("docs/guide")),
                "/images/photo.jpg"
            );
            assert_eq!(pr.web_path("/", Some("any/path")), "/");

            // No start path scenarios.
            assert_eq!(pr.web_path("images/photo.jpg", None), "images/photo.jpg");
            assert_eq!(pr.web_path("../photo.jpg", None), "../photo.jpg");

            // Path normalization with dots.
            assert_eq!(
                pr.web_path("./photo.jpg", Some("images")),
                "images/photo.jpg"
            );
            assert_eq!(
                pr.web_path("folder/./photo.jpg", Some("images")),
                "images/folder/photo.jpg"
            );
            assert_eq!(
                pr.web_path("folder/../photo.jpg", Some("images")),
                "images/photo.jpg"
            );

            // Complex path resolution.
            assert_eq!(
                pr.web_path("../../../photo.jpg", Some("docs/images/folder/sub")),
                "docs/photo.jpg"
            );
            assert_eq!(
                pr.web_path("folder/../../photo.jpg", Some("docs/images")),
                "docs/photo.jpg"
            );
            assert_eq!(
                pr.web_path("./folder/../photo.jpg", Some("images")),
                "images/photo.jpg"
            );

            // Edge cases with trailing slashes.
            assert_eq!(
                pr.web_path("photo.jpg", Some("images/")),
                "images/photo.jpg"
            );
            assert_eq!(pr.web_path("photo.jpg", Some("images")), "images/photo.jpg");

            // URLs with paths and parent references.
            assert_eq!(
                pr.web_path("../styles/main.css", Some("https://example.com/assets/css")),
                "https://example.com/assets/styles/main.css"
            );
            assert_eq!(
                pr.web_path(
                    "../../images/logo.png",
                    Some("http://site.com/docs/guide/examples")
                ),
                "http://site.com/docs/images/logo.png"
            );

            // Space handling (gets URL encoded).
            assert_eq!(
                pr.web_path("my file.jpg", Some("images")),
                "images/my%20file.jpg"
            );
            assert_eq!(
                pr.web_path("folder with spaces/file.jpg", Some("docs")),
                "docs/folder%20with%20spaces/file.jpg"
            );

            // Protocol-less absolute paths.
            assert_eq!(
                pr.web_path(
                    "//cdn.example.com/assets/image.jpg",
                    Some("http://example.com")
                ),
                "//cdn.example.com/assets/image.jpg"
            );

            // Mixed scenarios. (An empty target resolves to the start path with
            // no spurious trailing slash, and an empty `start` is treated like
            // `None` — matching Ruby's `web_path`; see `paths_test.rb`.)
            assert_eq!(pr.web_path("", Some("docs/images")), "docs/images");
            assert_eq!(pr.web_path("", Some("")), "");
            assert_eq!(pr.web_path("", None), "");

            // Complex URI scenarios.
            assert_eq!(
                pr.web_path("api/v1/data", Some("https://api.example.com:8080/base")),
                "https://api.example.com:8080/base/api/v1/data"
            );
            assert_eq!(
                pr.web_path("../v2/data", Some("https://api.example.com/api/v1")),
                "https://api.example.com/api/v2/data"
            );

            // File protocol variations.
            assert_eq!(
                pr.web_path("document.pdf", Some("file:///C:/Users/docs")),
                "file:///C:/Users/docs/document.pdf"
            );
            assert_eq!(
                pr.web_path("../shared/doc.pdf", Some("file:///home/user/documents")),
                "file:///home/user/shared/doc.pdf"
            );
        }
    }

    #[test]
    fn is_web_root() {
        let pr = PathResolver::default();
        assert!(pr.is_web_root("/blah"));
        assert!(!pr.is_web_root(""));
        assert!(!pr.is_web_root("./blah"));
    }

    mod partition_path {
        use super::super::WebPath;
        use crate::parser::PathResolver;

        fn seg(items: &[&str]) -> Vec<String> {
            items.iter().map(|s| (*s).to_owned()).collect()
        }

        #[test]
        fn relative_system_path() {
            let pr = PathResolver::default();
            assert_eq!(
                pr.partition_path("sample/path", WebPath(false)),
                (seg(&["sample", "path"]), None)
            );
        }

        #[test]
        fn dot_slash_system_path() {
            let pr = PathResolver::default();
            assert_eq!(
                pr.partition_path("./sample/path", WebPath(false)),
                (seg(&["sample", "path"]), Some("./".to_owned()))
            );
        }

        #[test]
        fn self_references_are_removed() {
            let pr = PathResolver::default();
            assert_eq!(
                pr.partition_path("sample/./path", WebPath(false)),
                (seg(&["sample", "path"]), None)
            );
        }

        #[test]
        fn absolute_system_path() {
            let pr = PathResolver::default();
            assert_eq!(
                pr.partition_path("/sample/path", WebPath(false)),
                (seg(&["sample", "path"]), Some("/".to_owned()))
            );
        }

        #[test]
        fn unc_path() {
            let pr = PathResolver::default();
            assert_eq!(
                pr.partition_path("//server/share/path", WebPath(false)),
                (seg(&["server", "share", "path"]), Some("//".to_owned()))
            );
        }

        #[test]
        fn uri_classloader_rooted() {
            let pr = PathResolver::default();
            assert_eq!(
                pr.partition_path("uri:classloader:/sample/path", WebPath(false)),
                (
                    seg(&["", "sample", "path"]),
                    Some("uri:classloader:".to_owned())
                )
            );
        }

        #[test]
        fn uri_classloader_relative() {
            let pr = PathResolver::default();
            assert_eq!(
                pr.partition_path("uri:classloader:sample/path", WebPath(false)),
                (
                    seg(&["sample", "path"]),
                    Some("uri:classloader:".to_owned())
                )
            );
        }

        #[test]
        fn windows_drive_letter() {
            let pr = PathResolver {
                file_separator: '\\',
            };

            assert_eq!(
                pr.partition_path("C:\\sample\\path", WebPath(false)),
                (seg(&["sample", "path"]), Some("C:/".to_owned()))
            );
        }

        #[test]
        fn windows_drive_letter_forward_slashes() {
            let pr = PathResolver {
                file_separator: '\\',
            };

            assert_eq!(
                pr.partition_path("C:/sample/path", WebPath(false)),
                (seg(&["sample", "path"]), Some("C:/".to_owned()))
            );
        }

        #[test]
        fn drive_letter_is_relative_on_posix_resolver() {
            // Without a Windows-style separator, a drive-letter prefix is not
            // treated as a root. (Constructed explicitly rather than via
            // `default()`, whose separator is platform-dependent.)
            let pr = PathResolver {
                file_separator: '/',
            };

            assert_eq!(
                pr.partition_path("C:/sample/path", WebPath(false)),
                (seg(&["C:", "sample", "path"]), None)
            );
        }
    }
}
