//! XPath-like query support for Virtual DOM nodes.
//!
//! This module provides a minimal XPath query engine for testing purposes.
//! It supports a subset of XPath syntax used in test assertions.

use crate::tests::assert_dom::virtual_dom::VirtualNode;

/// Removes duplicate nodes from a vector by comparing pointers.
/// Preserves document order (first occurrence is kept).
fn deduplicate_nodes(nodes: Vec<&VirtualNode>) -> Vec<&VirtualNode> {
    let mut seen: Vec<*const VirtualNode> = Vec::new();
    let mut result = Vec::new();
    for node in nodes {
        let ptr = node as *const VirtualNode;
        if !seen.contains(&ptr) {
            seen.push(ptr);
            result.push(node);
        }
    }
    result
}

/// Queries a virtual DOM tree using an XPath-like selector.
///
/// Supports the following patterns:
/// - `//tag` - Find all elements with the given tag anywhere in the tree
/// - `/tag` - Find tag elements as direct children of root
/// - `//tag/child` - Find child elements as direct children of tag elements
/// - `//tag[@attr="value"]` - Find elements with specific attribute values
/// - `//tag[text()="value"]` - Find elements with specific text content
/// - `//tag[N]` - Find the Nth element (1-indexed)
/// - `//tag[N]/*` - Find all children of the Nth element
/// - `//*` or `*` - Match any element
/// - `(//tag)[N]/child` - Apply predicate to subquery results
/// - `//tag/preceding-sibling::*` - Find preceding siblings of matched elements
/// - `//tag/following-sibling::*` - Find following siblings of matched elements
///
/// # Example
///
/// ```ignore
/// let doc = Parser::default().parse("* item 1\n* item 2");
/// let vdom = doc.to_virtual_dom();
/// let items = query_xpath(&vdom, "//ul/li");
/// assert_eq!(items.len(), 2);
/// ```
pub(crate) fn query_xpath<'a>(root: &'a VirtualNode, xpath: &str) -> Vec<&'a VirtualNode> {
    let xpath = xpath.trim();

    // Handle parenthesized subqueries: (//tag)[N]/rest
    if xpath.starts_with('(') {
        return query_parenthesized(root, xpath);
    }

    // Handle descendant-or-self pattern: //tag
    if let Some(rest) = xpath.strip_prefix("//") {
        return query_descendant_or_self(root, rest);
    }

    // Handle root-relative pattern: /tag
    if let Some(rest) = xpath.strip_prefix('/') {
        return query_from_root(root, rest);
    }

    // Default: treat as descendant-or-self.
    query_descendant_or_self(root, xpath)
}

/// Finds the matching closing parenthesis for an opening paren at position 0.
/// Returns the index of the matching ')' or None if not found.
fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, ch) in s.chars().enumerate() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Handles parenthesized subqueries like `(//ul)[1]/li`.
fn query_parenthesized<'a>(root: &'a VirtualNode, xpath: &str) -> Vec<&'a VirtualNode> {
    // Find the matching closing parenthesis (handling nested parens).
    let close_paren = find_matching_paren(xpath);
    if let Some(close_paren) = close_paren {
        let subquery = &xpath[1..close_paren]; // Extract content between parentheses.
        let rest = &xpath[close_paren + 1..].trim_start(); // Everything after ).

        // Execute the subquery.
        let mut results = query_xpath(root, subquery);

        // Check if there's a predicate immediately after the closing paren.
        if let Some(rest) = rest.strip_prefix('[') {
            if let Some(bracket_end) = rest.find(']') {
                let predicate = &rest[..bracket_end];
                let remaining = &rest[bracket_end + 1..];

                // Apply numeric predicate.
                if let Ok(index) = predicate.trim().parse::<usize>() {
                    if index > 0 && index <= results.len() {
                        results = vec![results[index - 1]];
                    } else {
                        return vec![];
                    }
                }

                // Continue with the remaining path if any.
                if !remaining.is_empty() {
                    // Check if remaining starts with another predicate (e.g., [text()="value"])
                    if remaining.starts_with('[') {
                        // Extract consecutive predicates.
                        let (predicates, path_after) = extract_predicates(remaining);

                        // Apply the predicates to filter the results.
                        results.retain(|node| matches_predicate(node, predicates));

                        // If there's more path after the predicates, continue processing.
                        if !path_after.is_empty() {
                            let mut final_results = Vec::new();
                            for node in results {
                                if path_after.starts_with('/') && !path_after.starts_with("//") {
                                    // Direct child path.
                                    final_results.extend(query_from_root(node, &path_after[1..]));
                                } else if let Some(stripped) = path_after.strip_prefix("//") {
                                    // Descendant path.
                                    final_results.extend(query_descendant_or_self(node, stripped));
                                }
                            }
                            return final_results;
                        }

                        return results;
                    }

                    // Check for axis specifiers first.
                    if let Some(axis_rest) = remaining.strip_prefix("/preceding-sibling::") {
                        // Parse the axis_rest to see if there's a continuation after the axis.
                        let (axis_selector, continuation) =
                            if let Some(slash_pos) = axis_rest.find('/') {
                                (&axis_rest[..slash_pos], Some(&axis_rest[slash_pos..]))
                            } else {
                                (axis_rest.trim(), None)
                            };

                        let mut final_results = Vec::new();
                        for node in results {
                            let siblings = find_preceding_siblings(root, node, axis_selector);

                            // If there's a continuation, query each sibling.
                            if let Some(cont) = continuation {
                                for sibling in siblings {
                                    if let Some(stripped) = cont.strip_prefix("//") {
                                        final_results
                                            .extend(query_descendant_or_self(sibling, stripped));
                                    } else if let Some(stripped) = cont.strip_prefix('/') {
                                        final_results.extend(query_from_root(sibling, stripped));
                                    }
                                }
                            } else {
                                final_results.extend(siblings);
                            }
                        }
                        return final_results;
                    }

                    if let Some(axis_rest) = remaining.strip_prefix("/following-sibling::") {
                        // Parse the axis_rest to see if there's a continuation after the axis.
                        let (axis_selector, continuation) =
                            if let Some(slash_pos) = axis_rest.find('/') {
                                (&axis_rest[..slash_pos], Some(&axis_rest[slash_pos..]))
                            } else {
                                (axis_rest.trim(), None)
                            };

                        let mut final_results = Vec::new();
                        for node in results {
                            let siblings = find_following_siblings(root, node, axis_selector);

                            // If there's a continuation, query each sibling.
                            if let Some(cont) = continuation {
                                for sibling in siblings {
                                    if let Some(stripped) = cont.strip_prefix("//") {
                                        final_results
                                            .extend(query_descendant_or_self(sibling, stripped));
                                    } else if let Some(stripped) = cont.strip_prefix('/') {
                                        final_results.extend(query_from_root(sibling, stripped));
                                    }
                                }
                            } else {
                                final_results.extend(siblings);
                            }
                        }
                        return final_results;
                    }

                    if let Some(axis_rest) = remaining.strip_prefix("/self::") {
                        // The `self::` axis tests if the current node matches the given selector.
                        // Parse to separate the selector (with predicates) from any continuation
                        // path.
                        let (selector, predicate, continuation) =
                            parse_selector_with_predicates(axis_rest);

                        let selector_with_pred = if let Some(pred) = predicate {
                            format!("{}{}", selector, pred)
                        } else {
                            selector.to_string()
                        };

                        // Filter results to only nodes matching the selector.
                        results.retain(|node| matches_selector(node, &selector_with_pred));

                        // If there's a continuation path, apply it to the filtered results.
                        if let Some(cont) = continuation {
                            let mut final_results = Vec::new();
                            for node in results {
                                if let Some(stripped) = cont.strip_prefix("//") {
                                    final_results.extend(query_descendant_or_self(node, stripped));
                                } else if let Some(stripped) = cont.strip_prefix('/') {
                                    final_results.extend(query_from_root(node, stripped));
                                }
                            }
                            return final_results;
                        }

                        return results;
                    }

                    let mut final_results = Vec::new();
                    for node in results {
                        if remaining.starts_with('/') && !remaining.starts_with("//") {
                            // Direct child path.
                            final_results.extend(query_from_root(node, &remaining[1..]));
                        } else if let Some(stripped) = remaining.strip_prefix("//") {
                            // Descendant path.
                            final_results.extend(query_descendant_or_self(node, stripped));
                        }
                    }
                    return final_results;
                }
            }
        } else if !rest.is_empty() {
            // Check if rest is a preceding-sibling axis.
            if let Some(axis_rest) = rest.strip_prefix("/preceding-sibling::") {
                // Parse the axis_rest to see if there's a continuation after the axis.
                let (axis_selector, continuation) = if let Some(slash_pos) = axis_rest.find('/') {
                    (&axis_rest[..slash_pos], Some(&axis_rest[slash_pos..]))
                } else {
                    (axis_rest.trim(), None)
                };

                let mut final_results = Vec::new();
                for node in results {
                    let siblings = find_preceding_siblings(root, node, axis_selector);

                    // If there's a continuation, query each sibling.
                    if let Some(cont) = continuation {
                        for sibling in siblings {
                            if let Some(stripped) = cont.strip_prefix("//") {
                                final_results.extend(query_descendant_or_self(sibling, stripped));
                            } else if let Some(stripped) = cont.strip_prefix('/') {
                                final_results.extend(query_from_root(sibling, stripped));
                            }
                        }
                    } else {
                        final_results.extend(siblings);
                    }
                }
                return final_results;
            }

            // Check if rest is a following-sibling axis.
            if let Some(axis_rest) = rest.strip_prefix("/following-sibling::") {
                // Parse the axis_rest to see if there's a continuation after the axis.
                let (axis_selector, continuation) = if let Some(slash_pos) = axis_rest.find('/') {
                    (&axis_rest[..slash_pos], Some(&axis_rest[slash_pos..]))
                } else {
                    (axis_rest.trim(), None)
                };

                let mut final_results = Vec::new();
                for node in results {
                    let siblings = find_following_siblings(root, node, axis_selector);

                    // If there's a continuation, query each sibling.
                    if let Some(cont) = continuation {
                        for sibling in siblings {
                            if let Some(stripped) = cont.strip_prefix("//") {
                                final_results.extend(query_descendant_or_self(sibling, stripped));
                            } else if let Some(stripped) = cont.strip_prefix('/') {
                                final_results.extend(query_from_root(sibling, stripped));
                            }
                        }
                    } else {
                        final_results.extend(siblings);
                    }
                }
                return final_results;
            }

            // Check if rest is a `self` axis.
            if let Some(axis_rest) = rest.strip_prefix("/self::") {
                // The `self::` axis tests if the current node matches the given selector.
                // Parse to separate the selector (with predicates) from any continuation path.
                let (selector, predicate, continuation) = parse_selector_with_predicates(axis_rest);

                let selector_with_pred = if let Some(pred) = predicate {
                    format!("{}{}", selector, pred)
                } else {
                    selector.to_string()
                };

                // Filter results to only nodes matching the selector.
                results.retain(|node| matches_selector(node, &selector_with_pred));

                // If there's a continuation path, apply it to the filtered results.
                if let Some(cont) = continuation {
                    let mut final_results = Vec::new();
                    for node in results {
                        if let Some(stripped) = cont.strip_prefix("//") {
                            final_results.extend(query_descendant_or_self(node, stripped));
                        } else if let Some(stripped) = cont.strip_prefix('/') {
                            final_results.extend(query_from_root(node, stripped));
                        }
                    }
                    return final_results;
                }

                return results;
            }

            // Continue with the remaining path without a predicate.
            let mut final_results = Vec::new();
            for node in results {
                if rest.starts_with('/') && !rest.starts_with("//") {
                    // Direct child path.
                    final_results.extend(query_from_root(node, &rest[1..]));
                } else if let Some(stripped) = rest.strip_prefix("//") {
                    // Descendant path.
                    final_results.extend(query_descendant_or_self(node, stripped));
                }
            }
            return final_results;
        }

        return results;
    }

    // No valid parenthesized expression found.
    vec![]
}

/// Parses a selector to extract base selector, predicates, and any continuation
/// path.
///
/// For example, `*[@class="foo"]//p[text()="bar"]` returns:
/// - base_selector: `*`
/// - predicate_part: Some(`[@class="foo"]`)
/// - continuation: Some(`//p[text()="bar"]`)
fn parse_selector_with_predicates(pattern: &str) -> (&str, Option<&str>, Option<&str>) {
    let mut base_end = 0;
    let mut predicate_start: Option<usize> = None;
    let mut predicate_end: Option<usize> = None;
    let mut bracket_depth = 0;
    let mut in_string = false;
    let mut string_delim = '\0';

    for (i, ch) in pattern.char_indices() {
        match ch {
            '[' if !in_string => {
                if bracket_depth == 0 && predicate_start.is_none() {
                    predicate_start = Some(i);
                    base_end = i;
                }
                bracket_depth += 1;
            }

            ']' if !in_string => {
                bracket_depth -= 1;
                if bracket_depth == 0 {
                    predicate_end = Some(i + 1);
                }
            }

            '"' | '\'' if bracket_depth > 0 => {
                if !in_string {
                    in_string = true;
                    string_delim = ch;
                } else if ch == string_delim {
                    in_string = false;
                }
            }

            '/' if bracket_depth == 0 && !in_string => {
                // Found a path separator outside of predicates.
                // Everything from here is a continuation.
                let base = if base_end > 0 {
                    &pattern[..base_end]
                } else {
                    &pattern[..i]
                };

                let pred = if let (Some(start), Some(end)) = (predicate_start, predicate_end) {
                    Some(&pattern[start..end])
                } else {
                    None
                };
                return (base, pred, Some(&pattern[i..]));
            }
            _ => {}
        }
    }

    // No continuation found.
    if let (Some(start), Some(end)) = (predicate_start, predicate_end) {
        (&pattern[..base_end], Some(&pattern[start..end]), None)
    } else if base_end > 0 {
        (&pattern[..base_end], None, None)
    } else {
        (pattern, None, None)
    }
}

/// Queries for descendants or self matching the pattern.
fn query_descendant_or_self<'a>(node: &'a VirtualNode, pattern: &str) -> Vec<&'a VirtualNode> {
    // Use parse_selector_with_predicates to properly handle '/' inside quoted
    // strings.
    let (base_selector, predicate_part, continuation) = parse_selector_with_predicates(pattern);

    // If there's a continuation (path separator found), handle it.
    if let Some(cont) = continuation {
        // Combine base selector and predicate to get the full first part.
        let first_str = if let Some(pred) = predicate_part {
            format!("{}{}", base_selector.trim(), pred)
        } else {
            base_selector.trim().to_string()
        };

        let first = first_str.as_str();

        // Determine if this is a descendant (//) or direct child (/) path.
        // `cont` starts with at least one '/'. Check if it's '//'.
        let is_descendant_path = cont.starts_with("//");
        let rest = cont.trim_start_matches('/');

        // Check for axis specifiers like "preceding-sibling::" or
        // "following-sibling::".
        if let Some(axis_rest) = rest.strip_prefix("preceding-sibling::") {
            // Find all nodes matching first part.
            let mut results = Vec::new();
            collect_descendants_matching(node, first, &mut results);
            results = apply_numeric_predicate(results, first);

            // Parse axis_rest to separate the sibling selector from any continuation.
            let (axis_selector, axis_predicate, continuation) =
                parse_selector_with_predicates(axis_rest);
            let axis_selector_with_pred = if let Some(pred) = axis_predicate {
                format!("{}{}", axis_selector, pred)
            } else {
                axis_selector.to_string()
            };

            // For each matched node, find its preceding siblings.
            let mut final_results = Vec::new();
            for matched_node in results {
                let siblings =
                    find_preceding_siblings(node, matched_node, &axis_selector_with_pred);

                // If there's a continuation, query each sibling.
                if let Some(cont) = continuation {
                    for sibling in siblings {
                        if let Some(stripped) = cont.strip_prefix("//") {
                            final_results.extend(query_descendant_or_self(sibling, stripped));
                        } else if let Some(stripped) = cont.strip_prefix('/') {
                            final_results.extend(query_from_root(sibling, stripped));
                        }
                    }
                } else {
                    final_results.extend(siblings);
                }
            }
            // Deduplicate results since the same sibling may be found from multiple nodes.
            return deduplicate_nodes(final_results);
        }

        if let Some(axis_rest) = rest.strip_prefix("following-sibling::") {
            // Find all nodes matching first part.
            let mut results = Vec::new();
            collect_descendants_matching(node, first, &mut results);
            results = apply_numeric_predicate(results, first);

            // Parse axis_rest to separate the sibling selector from any continuation.
            let (axis_selector, axis_predicate, continuation) =
                parse_selector_with_predicates(axis_rest);
            let axis_selector_with_pred = if let Some(pred) = axis_predicate {
                format!("{}{}", axis_selector, pred)
            } else {
                axis_selector.to_string()
            };

            // For each matched node, find its following siblings.
            let mut final_results = Vec::new();
            for matched_node in results {
                let siblings =
                    find_following_siblings(node, matched_node, &axis_selector_with_pred);

                // If there's a continuation, query each sibling.
                if let Some(cont) = continuation {
                    for sibling in siblings {
                        if let Some(stripped) = cont.strip_prefix("//") {
                            final_results.extend(query_descendant_or_self(sibling, stripped));
                        } else if let Some(stripped) = cont.strip_prefix('/') {
                            final_results.extend(query_from_root(sibling, stripped));
                        }
                    }
                } else {
                    final_results.extend(siblings);
                }
            }
            // Deduplicate results since the same sibling may be found from multiple nodes.
            return deduplicate_nodes(final_results);
        }

        // Find all nodes matching first part.
        let mut results = Vec::new();
        collect_descendants_matching(node, first, &mut results);

        // Apply numeric predicate if present.
        results = apply_numeric_predicate(results, first);

        // For each matching node, query its children with the rest of the path.
        let mut final_results = Vec::new();
        for matched_node in results {
            if is_descendant_path {
                // Descendant pattern continues: //
                let descendants = query_descendant_or_self(matched_node, rest);
                final_results.extend(descendants);
            } else {
                // Direct child pattern: continue with rest as a path from this node.
                let children_results = query_from_root(matched_node, rest);
                final_results.extend(children_results);
            }
        }

        // Deduplicate since the same node may be reachable from multiple ancestors.
        deduplicate_nodes(final_results)
    } else {
        // Simple tag match or tag with predicate.
        // Extract base selector and predicate parts.
        let pattern = pattern.trim();

        // Parse predicates carefully to stop at // or / that appears outside brackets.
        let (base_selector, predicate_part, continuation) = parse_selector_with_predicates(pattern);

        let mut results = Vec::new();
        collect_descendants_matching(node, base_selector, &mut results);

        // If there are predicates, filter results by predicates.
        if let Some(pred) = predicate_part {
            results.retain(|n| matches_predicate(n, pred));
        }

        // If there's a continuation path (e.g., //p[text()="numbered"]),
        // query descendants of each result.
        if let Some(cont) = continuation {
            let mut final_results = Vec::new();
            for matched_node in results {
                if let Some(stripped) = cont.strip_prefix("//") {
                    final_results.extend(query_descendant_or_self(matched_node, stripped));
                } else if let Some(stripped) = cont.strip_prefix('/') {
                    final_results.extend(query_from_root(matched_node, stripped));
                }
            }
            return apply_numeric_predicate(final_results, pattern);
        }

        apply_numeric_predicate(results, pattern)
    }
}

/// Queries from root using direct child selectors.
fn query_from_root<'a>(node: &'a VirtualNode, pattern: &str) -> Vec<&'a VirtualNode> {
    // Check for axis specifiers first.
    if let Some(axis_rest) = pattern.strip_prefix("following-sibling::") {
        return find_following_siblings(node, node, axis_rest.trim());
    }

    if let Some(axis_rest) = pattern.strip_prefix("preceding-sibling::") {
        return find_preceding_siblings(node, node, axis_rest.trim());
    }

    // Check for descendant-or-self pattern (//). Handle this before split_once
    // to preserve the // distinction.
    if let Some(descendant_rest) = pattern.strip_prefix("//") {
        // Pattern starts with //, so this is a descendant-or-self query.
        return query_descendant_or_self(node, descendant_rest);
    }

    if let Some((first, rest)) = pattern.split_once('/') {
        let first = first.trim();
        let rest = rest.trim();

        // Check if rest is an axis specifier.
        if let Some(axis_rest) = rest.strip_prefix("following-sibling::") {
            // Parse the axis_rest to see if there's a continuation after the axis.
            let (axis_selector, continuation) = if let Some(slash_pos) = axis_rest.find('/') {
                (&axis_rest[..slash_pos], Some(&axis_rest[slash_pos..]))
            } else {
                (axis_rest.trim(), None)
            };

            let mut final_results = Vec::new();
            for child in &node.children {
                if matches_selector(child, first) {
                    let siblings = find_following_siblings(node, child, axis_selector);

                    // If there's a continuation, query each sibling.
                    if let Some(cont) = continuation {
                        for sibling in siblings {
                            if let Some(stripped) = cont.strip_prefix("//") {
                                final_results.extend(query_descendant_or_self(sibling, stripped));
                            } else if let Some(stripped) = cont.strip_prefix('/') {
                                final_results.extend(query_from_root(sibling, stripped));
                            }
                        }
                    } else {
                        final_results.extend(siblings);
                    }
                }
            }
            // Deduplicate results since the same sibling may be found from multiple nodes.
            return deduplicate_nodes(final_results);
        }

        if let Some(axis_rest) = rest.strip_prefix("preceding-sibling::") {
            // Parse the axis_rest to see if there's a continuation after the axis.
            let (axis_selector, continuation) = if let Some(slash_pos) = axis_rest.find('/') {
                (&axis_rest[..slash_pos], Some(&axis_rest[slash_pos..]))
            } else {
                (axis_rest.trim(), None)
            };

            let mut final_results = Vec::new();
            for child in &node.children {
                if matches_selector(child, first) {
                    let siblings = find_preceding_siblings(node, child, axis_selector);

                    // If there's a continuation, query each sibling.
                    if let Some(cont) = continuation {
                        for sibling in siblings {
                            if let Some(stripped) = cont.strip_prefix("//") {
                                final_results.extend(query_descendant_or_self(sibling, stripped));
                            } else if let Some(stripped) = cont.strip_prefix('/') {
                                final_results.extend(query_from_root(sibling, stripped));
                            }
                        }
                    } else {
                        final_results.extend(siblings);
                    }
                }
            }
            // Deduplicate results since the same sibling may be found from multiple nodes.
            return deduplicate_nodes(final_results);
        }

        // First, collect all matching children.
        let mut matching_children: Vec<&VirtualNode> = node
            .children
            .iter()
            .filter(|child| matches_selector(child, first))
            .collect();

        // Apply numeric predicate to the matching children.
        matching_children = apply_numeric_predicate(matching_children, first.trim());

        // Now process each (possibly filtered) child.
        let mut results = Vec::new();
        for child in matching_children {
            if rest.is_empty() {
                results.push(child);
            } else if rest.starts_with('/') {
                // Continue with descendant-or-self.
                results.extend(query_descendant_or_self(
                    child,
                    rest.trim_start_matches('/'),
                ));
            } else {
                // Continue with direct children.
                results.extend(query_from_root(child, rest));
            }
        }

        results
    } else {
        // Direct children only.
        let results: Vec<&VirtualNode> = node
            .children
            .iter()
            .filter(|child| matches_selector(child, pattern.trim()))
            .collect();

        apply_numeric_predicate(results, pattern.trim())
    }
}

/// Recursively collects all descendants that match the selector.
/// Note: This does NOT include self - only true descendants.
fn collect_descendants_matching<'a>(
    node: &'a VirtualNode,
    selector: &str,
    results: &mut Vec<&'a VirtualNode>,
) {
    for child in &node.children {
        if matches_selector(child, selector) {
            results.push(child);
        }
        collect_descendants_matching(child, selector, results);
    }
}

/// Finds preceding siblings of a target node within the tree.
/// This searches the entire tree starting from root to find the parent of the
/// target, then returns all siblings that appear before the target.
fn find_preceding_siblings<'a>(
    root: &'a VirtualNode,
    target: &'a VirtualNode,
    selector: &str,
) -> Vec<&'a VirtualNode> {
    // Helper function to find the parent of a target node.
    fn find_parent(node: &VirtualNode, target: *const VirtualNode) -> Option<&VirtualNode> {
        for child in &node.children {
            if std::ptr::eq(child as *const _, target) {
                return Some(node);
            }
            if let Some(parent) = find_parent(child, target) {
                return Some(parent);
            }
        }
        None
    }

    // Find the parent of the target node.
    let target_ptr = target as *const VirtualNode;
    let parent = match find_parent(root, target_ptr) {
        Some(p) => p,
        None => return vec![], // Target not found in tree.
    };

    // Collect all preceding siblings that match the selector.
    let mut results = Vec::new();
    for child in &parent.children {
        // Stop when we reach the target node.
        if std::ptr::eq(child as *const _, target_ptr) {
            break;
        }
        // Add matching siblings.
        if matches_selector(child, selector) {
            results.push(child);
        }
    }

    results
}

/// Applies numeric predicate filtering (e.g., [1], [2]) to a results vector.
/// Returns the filtered results or the original results if no numeric predicate
/// is present.
fn apply_numeric_predicate<'a>(
    results: Vec<&'a VirtualNode>,
    selector: &str,
) -> Vec<&'a VirtualNode> {
    // Extract numeric predicate [N] from selector.
    if let Some(bracket_pos) = selector.find('[')
        && let Some(predicate) = selector[bracket_pos..]
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
    {
        // Check if it's a numeric predicate.
        if let Ok(index) = predicate.trim().parse::<usize>() {
            // XPath uses 1-based indexing.
            if index > 0 && index <= results.len() {
                return vec![results[index - 1]];
            } else {
                return vec![];
            }
        }
    }

    results
}

/// Finds following siblings of a target node within the tree.
/// This searches the entire tree starting from root to find the parent of the
/// target, then returns all siblings that appear after the target.
fn find_following_siblings<'a>(
    root: &'a VirtualNode,
    target: &'a VirtualNode,
    selector: &str,
) -> Vec<&'a VirtualNode> {
    // Helper function to find the parent of a target node.
    fn find_parent(node: &VirtualNode, target: *const VirtualNode) -> Option<&VirtualNode> {
        for child in &node.children {
            if std::ptr::eq(child as *const _, target) {
                return Some(node);
            }
            if let Some(parent) = find_parent(child, target) {
                return Some(parent);
            }
        }
        None
    }

    // Find the parent of the target node.
    let target_ptr = target as *const VirtualNode;
    let parent = match find_parent(root, target_ptr) {
        Some(p) => p,
        None => return vec![], // Target not found in tree.
    };

    // Collect all following siblings that match the selector.
    let mut results = Vec::new();
    let mut found_target = false;
    for child in &parent.children {
        // Start collecting after we've found the target node.
        if found_target {
            if matches_selector(child, selector) {
                results.push(child);
            }
        } else if std::ptr::eq(child as *const _, target_ptr) {
            found_target = true;
        }
    }

    results
}

/// Checks if a node matches the given selector.
///
/// Supports:
/// - Tag name: `div`, `ul`, `li`
/// - Wildcard: `*` (matches any element)
/// - Class selector: `[@class="ulist"]` or `.ulist`
/// - ID selector: `[@id="foo"]` or `#foo`
/// - Text content: `[text()="value"]`
/// - Index: `[1]`, `[2]`, etc. (handled by `apply_numeric_predicate`)
fn matches_selector(node: &VirtualNode, selector: &str) -> bool {
    let selector = selector.trim();

    // Handle index predicates [N] by stripping them off.
    // (Caller should handle filtering by index.)
    let (base_selector, predicate) = if let Some(bracket_pos) = selector.find('[') {
        (&selector[..bracket_pos], Some(&selector[bracket_pos..]))
    } else {
        (selector, None)
    };

    // Wildcard selector: matches any element (but not text nodes).
    if base_selector == "*" {
        // Exclude text nodes from wildcard matches.
        if node.tag == "text" {
            return false;
        }
        if let Some(predicate) = predicate {
            return matches_predicate(node, predicate);
        }
        return true;
    }

    // CSS-style class selector: `.classname`
    if let Some(class_name) = base_selector.strip_prefix('.') {
        return node.classes.iter().any(|c| c == class_name);
    }

    // CSS-style ID selector: `#id`
    if let Some(id) = base_selector.strip_prefix('#') {
        return node.id.as_deref() == Some(id);
    }

    // Tag name match.
    if !base_selector.is_empty() && node.tag != base_selector {
        return false;
    }

    // Handle predicates if present.
    if let Some(predicate) = predicate {
        return matches_predicate(node, predicate);
    }

    true
}

/// Extracts consecutive predicates from a path string.
///
/// Given a string like `[@id="beck"][@class="title"]/div/span`, returns
/// `("[@id=\"beck\"][@class=\"title\"]", "/div/span")`.
fn extract_predicates(path: &str) -> (&str, &str) {
    let path = path.trim();
    let mut end = 0;
    let mut in_predicate = false;
    let mut bracket_depth = 0;

    for (i, ch) in path.chars().enumerate() {
        match ch {
            '[' => {
                in_predicate = true;
                bracket_depth += 1;
            }
            ']' => {
                bracket_depth -= 1;
                if bracket_depth == 0 {
                    end = i + 1;
                    in_predicate = false;
                }
            }
            '/' if !in_predicate && bracket_depth == 0 => {
                // Found a path separator after all predicates.
                return (&path[..end], &path[end..]);
            }
            _ => {}
        }
    }

    // All predicates, no path after.
    (&path[..end], &path[end..])
}

/// Checks if a node matches a predicate like `[@class="value"]` or
/// `[text()="value"]`.
/// Can handle multiple predicates like `[@class="value"][text()="text"]`.
fn matches_predicate(node: &VirtualNode, predicate: &str) -> bool {
    let mut predicate = predicate.trim();

    // Handle multiple predicates by checking each one.
    while !predicate.is_empty() {
        // Find the next opening bracket.
        if let Some(bracket_start) = predicate.find('[') {
            // Find the matching closing bracket, respecting quoted strings.
            if let Some(bracket_end) =
                find_closing_bracket(&predicate[bracket_start..]).map(|i| bracket_start + i)
            {
                let single_pred = &predicate[bracket_start + 1..bracket_end];

                // Check this single predicate.
                if !matches_single_predicate(node, single_pred) {
                    return false;
                }

                // Move to the next predicate.
                predicate = predicate[bracket_end + 1..].trim();
            } else {
                // Malformed predicate.
                return false;
            }
        } else {
            // No more predicates.
            break;
        }
    }

    true
}

/// Finds the byte position of the closing bracket `]` that matches the opening
/// `[`, while respecting quoted strings (both single and double quotes).
/// Returns the byte index relative to the start of the input string.
fn find_closing_bracket(s: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = '\0';

    // Use char_indices() to get byte positions instead of character indices.
    for (byte_pos, ch) in s.char_indices() {
        // Handle escape sequences.
        if prev_char == '\\' {
            prev_char = '\0'; // Reset to avoid double-escape issues.
            continue;
        }

        match ch {
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '[' if !in_single_quote && !in_double_quote => depth += 1,
            ']' if !in_single_quote && !in_double_quote => {
                depth -= 1;
                if depth == 0 {
                    return Some(byte_pos);
                }
            }
            _ => {}
        }

        prev_char = ch;
    }

    None
}

/// Gets the text content of a node, recursively collecting text from all
/// child nodes.
///
/// This mimics the behavior of the XPath `text()` function or DOM's
/// `textContent`.
fn get_text_content(node: &VirtualNode) -> String {
    if let Some(ref text) = node.text {
        // Node has direct text content.
        return text.clone();
    }

    // No direct text - collect from children.
    let mut result = String::new();
    for child in &node.children {
        result.push_str(&get_text_content(child));
    }
    result
}

/// Checks if a node matches a single predicate.
fn matches_single_predicate(node: &VirtualNode, predicate: &str) -> bool {
    let predicate = predicate.trim();

    // Check for `normalize-space(text())` predicate.
    if let Some(rest) = predicate.strip_prefix("normalize-space(text())") {
        let rest = rest.trim();

        // Handle normalize-space(text()) = 'value' or = "value".
        if let Some(value_part) = rest.strip_prefix('=').map(|s| s.trim()) {
            // Try single-quoted string first.
            if let Some(value) = value_part.strip_prefix('\'') {
                if let Some(value) = value.strip_suffix('\'') {
                    let unescaped = unescape_xpath_string(value);
                    return normalize_space(&get_text_content(node)) == unescaped;
                }
            }
            // Try double-quoted string.
            else if let Some(value) = value_part.strip_prefix('"')
                && let Some(value) = value.strip_suffix('"')
            {
                let unescaped = unescape_xpath_string(value);
                return normalize_space(&get_text_content(node)) == unescaped;
            }
        }

        return false;
    }

    // Check for `starts-with()` predicate.
    if let Some(rest) = predicate.strip_prefix("starts-with(") {
        // Parse starts-with(text(), "value").
        let rest = rest.trim();
        if let Some(args) = rest.strip_prefix("text()") {
            let args = args.trim();
            if let Some(args) = args.strip_prefix(',') {
                let args = args.trim();
                // Find the closing parenthesis for starts-with().
                if let Some(close_paren) = args.rfind(')') {
                    let value_part = args[..close_paren].trim();
                    // Try double-quoted string.
                    if let Some(value) = value_part.strip_prefix('"')
                        && let Some(value) = value.strip_suffix('"')
                    {
                        let unescaped = unescape_xpath_string(value);
                        return get_text_content(node).starts_with(&unescaped);
                    }
                    // Try single-quoted string.
                    else if let Some(value) = value_part.strip_prefix('\'')
                        && let Some(value) = value.strip_suffix('\'')
                    {
                        let unescaped = unescape_xpath_string(value);
                        return get_text_content(node).starts_with(&unescaped);
                    }
                }
            }
        }
        return false;
    }

    // Check for `text()` predicate.
    if let Some(rest) = predicate.strip_prefix("text()") {
        let rest = rest.trim();

        // Handle text() = 'value' (single quotes).
        if let Some(value_part) = rest.strip_prefix('=').map(|s| s.trim()) {
            // Try single-quoted string first.
            if let Some(value) = value_part.strip_prefix('\'') {
                if let Some(value) = value.strip_suffix('\'') {
                    let unescaped = unescape_xpath_string(value);
                    return get_text_content(node) == unescaped;
                }
            }
            // Try double-quoted string.
            else if let Some(value) = value_part.strip_prefix('"')
                && let Some(value) = value.strip_suffix('"')
            {
                let unescaped = unescape_xpath_string(value);
                return get_text_content(node) == unescaped;
            }
        }

        return false;
    }

    // Check for attribute predicates `[@attr="value"]`.
    if let Some(attr_part) = predicate.strip_prefix('@')
        && let Some((attr_name, value_part)) = attr_part.split_once('=')
    {
        let attr_name = attr_name.trim();
        let value = value_part
            .trim()
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(value_part.trim());

        match attr_name {
            "class" => {
                // Split the value by whitespace to handle multiple classes.
                // The node must have ALL classes specified in the value.
                let required_classes: Vec<&str> = value.split_whitespace().collect();
                return required_classes
                    .iter()
                    .all(|required| node.classes.iter().any(|c| c == *required));
            }
            "id" => return node.id.as_deref() == Some(value),
            _ => {
                // Check arbitrary attributes.
                return node.attributes.get(attr_name).map(|v| v.as_str()) == Some(value);
            }
        }
    }

    // Numeric predicate [N]: Would need to be handled by caller with context.
    // For now, just return `true` to pass through.
    true
}

/// Normalizes whitespace in a string, matching XPath normalize-space()
/// semantics. Strips leading/trailing whitespace and replaces sequences of
/// whitespace with a single space.
fn normalize_space(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Unescapes XPath string literals.
/// Handles escape sequences like `\n` (newline), `\'` (single quote), `\\`
/// (backslash).
fn unescape_xpath_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Handle escape sequence.
            if let Some(next) = chars.next() {
                match next {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '\'' => result.push('\''),
                    '"' => result.push('"'),
                    _ => {
                        // Unknown escape - keep as is.
                        result.push('\\');
                        result.push(next);
                    }
                }
            } else {
                result.push('\\');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Parser, tests::assert_dom::virtual_dom::ToVirtualDom};

    #[test]
    fn query_all_paragraphs() {
        let doc = Parser::default().parse("Para 1\n\nPara 2\n\nPara 3");
        let vdom = doc.to_virtual_dom();
        let paras = query_xpath(&vdom, "//p");
        assert_eq!(paras.len(), 3);
    }

    #[test]
    fn query_list_items() {
        let doc = Parser::default().parse("* item 1\n* item 2\n* item 3");
        let vdom = doc.to_virtual_dom();

        // Find all `ul` elements.
        let uls = query_xpath(&vdom, "//ul");
        assert_eq!(uls.len(), 1);

        // Find all `li` elements.
        let lis = query_xpath(&vdom, "//li");
        assert_eq!(lis.len(), 3);

        // Find `li` as children of `ul`.
        let ul_lis = query_xpath(&vdom, "//ul/li");
        assert_eq!(ul_lis.len(), 3);
    }

    #[test]
    fn query_with_class_selector() {
        let doc = Parser::default().parse("* item 1\n* item 2");
        let vdom = doc.to_virtual_dom();

        // CSS-style class selector>: Finds the wrapper `div`.
        let ulists = query_xpath(&vdom, "//.ulist");
        assert_eq!(ulists.len(), 1);
        assert_eq!(ulists[0].tag, "div"); // Wrapper `div` has the class.
    }

    #[test]
    fn query_section_headings() {
        let doc = Parser::default().parse("== Section 1\n\nPara\n\n== Section 2\n\nPara");
        let vdom = doc.to_virtual_dom();

        // Find all `h2` elements.
        let h2s = query_xpath(&vdom, "//h2");
        assert_eq!(h2s.len(), 2);
    }

    #[test]
    fn query_nested_path() {
        let doc = Parser::default().parse("== Section\n\n* item 1\n* item 2");
        let vdom = doc.to_virtual_dom();

        // First verify we can find the `ul`.
        let uls = query_xpath(&vdom, "//ul");
        assert_eq!(uls.len(), 1, "Should find one ul element");

        // Then verify we can find `ul/li`.
        let ul_lis = query_xpath(&vdom, "//ul/li");
        assert_eq!(ul_lis.len(), 2, "Should find 2 li elements under ul");

        // Find list items within any `div` that contains a `ul`.
        let items = query_xpath(&vdom, "//div/ul/li");
        assert_eq!(
            items.len(),
            2,
            "Should find 2 li elements via //div/ul/li path"
        );
    }

    #[test]
    fn query_with_text_predicate() {
        let doc = Parser::default().parse("Hello\n\nWorld");
        let vdom = doc.to_virtual_dom();

        let hello_para = query_xpath(&vdom, "//p[text()=\"Hello\"]");
        assert_eq!(hello_para.len(), 1);
        assert_eq!(hello_para[0].text.as_deref(), Some("Hello"));
    }

    #[test]
    fn query_with_attribute_predicate() {
        let doc = Parser::default().parse("* item");
        let vdom = doc.to_virtual_dom();

        // The wrapper `div` has the `ulist` class, not the `ul` element.
        let ulist_wrapper = query_xpath(&vdom, "//div[@class=\"ulist\"]");
        assert_eq!(ulist_wrapper.len(), 1);
    }

    #[test]
    fn query_with_numeric_predicate() {
        let doc = Parser::default().parse("* item 1\n* item 2\n* item 3");
        let vdom = doc.to_virtual_dom();

        let first_li = query_xpath(&vdom, "//ul/li[1]");
        assert_eq!(first_li.len(), 1);

        let second_li = query_xpath(&vdom, "//ul/li[2]");
        assert_eq!(second_li.len(), 1);

        let third_li = query_xpath(&vdom, "//ul/li[3]");
        assert_eq!(third_li.len(), 1);

        let fourth_li = query_xpath(&vdom, "//ul/li[4]");
        assert_eq!(fourth_li.len(), 0);
    }

    #[test]
    fn query_with_wildcard() {
        let doc = Parser::default().parse("* item 1\n* item 2");
        let vdom = doc.to_virtual_dom();

        let all_elements = query_xpath(&vdom, "//*");
        assert!(!all_elements.is_empty());

        let first_li_children = query_xpath(&vdom, "//ul/li[1]/*");
        assert_eq!(first_li_children.len(), 1);
        assert_eq!(first_li_children[0].tag, "p");
    }

    #[test]
    fn query_numeric_predicate_with_wildcard() {
        let doc = Parser::default().parse("* item 1\n* item 2\n* item 3");
        let vdom = doc.to_virtual_dom();

        let first_li_all_children = query_xpath(&vdom, "//ul/li[1]/*");
        assert_eq!(first_li_all_children.len(), 1);
        assert_eq!(first_li_all_children[0].tag, "p");

        let second_li_all_children = query_xpath(&vdom, "//ul/li[2]/*");
        assert_eq!(second_li_all_children.len(), 1);
        assert_eq!(second_li_all_children[0].tag, "p");
    }

    #[test]
    fn query_comprehensive_numeric_wildcard() {
        // This test demonstrates the full `//ul/li[1]/*` pattern.
        let doc = Parser::default().parse("* item 1\n* item 2\n* item 3");
        let vdom = doc.to_virtual_dom();

        // The pattern `//ul/li[1]/*` means:
        // 1. Find all <ul> elements (//ul)
        // 2. Get their <li> children (/li)
        // 3. Take only the first one ([1])
        // 4. Get all its children (/*)
        let result = query_xpath(&vdom, "//ul/li[1]/*");

        assert_eq!(result.len(), 1, "Should find exactly 1 child element");
        assert_eq!(result[0].tag, "p", "The child should be a paragraph");

        assert_eq!(
            result[0].text.as_deref(),
            Some("item 1"),
            "The paragraph should contain 'item 1'"
        );
    }

    #[test]
    fn query_parenthesized_with_predicate() {
        // Create a document with multiple lists separated by a comment.
        // This mimics the pattern from the actual test suite.
        let doc = Parser::default().parse("- Foo\n- Boo\n\n//\n\n- Blech\n");
        let vdom = doc.to_virtual_dom();

        let all_uls = query_xpath(&vdom, "//ul");
        assert_eq!(all_uls.len(), 2, "Should have 2 ul elements");

        let first_ul = query_xpath(&vdom, "(//ul)[1]");
        assert_eq!(first_ul.len(), 1, "Should find exactly 1 ul");

        let second_ul = query_xpath(&vdom, "(//ul)[2]");
        assert_eq!(second_ul.len(), 1, "Should find exactly 1 ul");

        let first_ul_lis = query_xpath(&vdom, "(//ul)[1]/li");
        assert_eq!(first_ul_lis.len(), 2, "First ul should have 2 li elements");

        let second_ul_lis = query_xpath(&vdom, "(//ul)[2]/li");
        assert_eq!(second_ul_lis.len(), 1, "Second ul should have 1 li element");
    }

    #[test]
    fn query_parenthesized_complex() {
        let doc = Parser::default().parse("- Foo\n- Boo\n\n.Also\n- Blech\n");
        let vdom = doc.to_virtual_dom();

        let all_uls = query_xpath(&vdom, "//ul");
        assert_eq!(all_uls.len(), 2);

        let result = query_xpath(&vdom, "(//ul)[1]/li[1]");
        assert_eq!(result.len(), 1);

        let result = query_xpath(&vdom, "(//ul)[2]/li");
        assert_eq!(result.len(), 1);

        let result = query_xpath(&vdom, "(//ul)[3]/li");
        assert_eq!(result.len(), 0, "Out of bounds should return empty");
    }

    #[test]
    fn query_parenthesized_complex_2() {
        let doc = Parser::default().parse("List\n====\n\n- Foo\n- Boo\n\n//\n\n- Blech\n");
        let vdom = doc.to_virtual_dom();

        assert_eq!(query_xpath(&vdom, "//ul").len(), 2);

        let first_ul_items = query_xpath(&vdom, "(//ul)[1]/li");
        assert_eq!(first_ul_items.len(), 2);

        let second_ul_items = query_xpath(&vdom, "(//ul)[2]/li");
        assert_eq!(second_ul_items.len(), 1);

        let all_items = query_xpath(&vdom, "//ul/li");
        assert_eq!(all_items.len(), 3);
    }

    #[test]
    fn query_text_predicate_with_newlines() {
        // Test text predicates with newlines using single quotes.
        let doc = Parser::default().parse("- Foo\nwrapped content\n");
        let vdom = doc.to_virtual_dom();

        // The text content should be "Foo\nwrapped content" (with actual newline).
        let result = query_xpath(&vdom, "//ul/li[1]/p[text() = 'Foo\nwrapped content']");
        assert_eq!(
            result.len(),
            1,
            "Should find paragraph with newline in text"
        );

        // Verify the actual text content.
        let para = query_xpath(&vdom, "//ul/li[1]/p");
        assert_eq!(para.len(), 1);
        assert_eq!(para[0].text.as_deref(), Some("Foo\nwrapped content"));
    }

    #[test]
    fn query_text_predicate_with_escaped_quotes() {
        // Test escaping single quotes within single-quoted strings.
        let doc = Parser::default().parse("Para with 'quotes'\n");
        let vdom = doc.to_virtual_dom();

        // Should match text with escaped single quote.
        let result = query_xpath(&vdom, "//p[text() = 'Para with \\'quotes\\'']");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn query_text_predicate_exact_match_from_test_suite() {
        // This exactly mirrors the test from lists_test.rs line 129.
        let doc = Parser::default().parse("List\n====\n\n- Foo\nwrapped content\n- Boo\n- Blech\n");
        let vdom = doc.to_virtual_dom();

        // Verify basic structure.
        assert_eq!(query_xpath(&vdom, "//ul").len(), 1);
        assert_eq!(query_xpath(&vdom, "//ul/li[1]/*").len(), 1);

        // The actual test from the suite.
        let result = query_xpath(&vdom, "//ul/li[1]/p[text() = 'Foo\nwrapped content']");
        assert_eq!(result.len(), 1, "Should match text with newline");
    }

    #[test]
    fn query_text_predicate_with_special_chars() {
        // Test with period at start (resembles block title).
        let doc = Parser::default().parse("== List\n\n- Foo\n.wrapped content\n- Boo\n- Blech\n");
        let vdom = doc.to_virtual_dom();

        let result = query_xpath(&vdom, "//ul/li[1]/p[text() = 'Foo\n.wrapped content']");
        assert_eq!(result.len(), 1);

        // Test with colon (resembles attribute entry).
        let doc2 = Parser::default().parse("== List\n\n- Foo\n:foo: bar\n- Boo\n- Blech\n");
        let vdom2 = doc2.to_virtual_dom();

        let result2 = query_xpath(&vdom2, "//ul/li[1]/p[text() = 'Foo\n:foo: bar']");
        assert_eq!(result2.len(), 1);
    }

    #[test]
    fn query_preceding_sibling_axis() {
        // Create a simple DOM structure with multiple siblings.
        let mut root = VirtualNode::new("div").with_class("document");
        root = root.with_child(VirtualNode::new("p").with_text("First"));
        root = root.with_child(VirtualNode::new("p").with_text("Second"));
        root = root.with_child(VirtualNode::new("p").with_text("Third"));

        // Find preceding siblings of the third paragraph.
        let result = query_xpath(&root, "//p[3]/preceding-sibling::p");
        assert_eq!(result.len(), 2, "Should find 2 preceding siblings");
        assert_eq!(result[0].text.as_deref(), Some("First"));
        assert_eq!(result[1].text.as_deref(), Some("Second"));
    }

    #[test]
    fn query_preceding_sibling_with_wildcard() {
        // Create a DOM with mixed element types.
        let mut root = VirtualNode::new("div").with_class("document");
        root = root.with_child(VirtualNode::new("p").with_text("Para"));
        root = root.with_child(
            VirtualNode::new("div")
                .with_class("title")
                .with_text("Title"),
        );
        root = root.with_child(VirtualNode::new("ul"));

        // Find all preceding siblings of ul.
        let result = query_xpath(&root, "//ul/preceding-sibling::*");
        assert_eq!(result.len(), 2, "Should find 2 preceding siblings");
        assert_eq!(result[0].tag, "p");
        assert_eq!(result[1].tag, "div");
    }

    #[test]
    fn query_preceding_sibling_with_predicate() {
        // Create a DOM with titled blocks.
        let mut root = VirtualNode::new("div").with_class("document");
        root = root.with_child(VirtualNode::new("div").with_class("ulist"));
        root = root.with_child(
            VirtualNode::new("div")
                .with_class("title")
                .with_text("Also"),
        );
        root = root.with_child(VirtualNode::new("div").with_class("ulist"));

        // Find preceding sibling with class="title" and text="Also".
        // Using parenthesized subquery to select the second ulist.
        let result = query_xpath(
            &root,
            "(//div[@class=\"ulist\"])[2]/preceding-sibling::*[@class=\"title\"][text()=\"Also\"]",
        );
        assert_eq!(result.len(), 1, "Should find the title element");
        assert_eq!(result[0].text.as_deref(), Some("Also"));
    }

    #[test]
    fn query_preceding_sibling_no_matches() {
        // Test when there are no matching preceding siblings.
        let mut root = VirtualNode::new("div").with_class("document");
        root = root.with_child(VirtualNode::new("p").with_text("Para"));
        root = root.with_child(VirtualNode::new("ul"));

        // The first element has no preceding siblings.
        let result = query_xpath(&root, "//p/preceding-sibling::*");
        assert_eq!(
            result.len(),
            0,
            "First element should have no preceding siblings"
        );
    }

    #[test]
    fn query_following_sibling_axis() {
        // Create a simple DOM structure with multiple siblings.
        let mut root = VirtualNode::new("div").with_class("document");
        root = root.with_child(VirtualNode::new("p").with_text("First"));
        root = root.with_child(VirtualNode::new("p").with_text("Second"));
        root = root.with_child(VirtualNode::new("p").with_text("Third"));

        // Find following siblings of the first paragraph.
        let result = query_xpath(&root, "//p[1]/following-sibling::p");
        assert_eq!(result.len(), 2, "Should find 2 following siblings");
        assert_eq!(result[0].text.as_deref(), Some("Second"));
        assert_eq!(result[1].text.as_deref(), Some("Third"));
    }

    #[test]
    fn query_following_sibling_with_wildcard() {
        // Create a DOM with mixed element types.
        let mut root = VirtualNode::new("div").with_class("document");
        root = root.with_child(VirtualNode::new("p").with_text("Para"));
        root = root.with_child(
            VirtualNode::new("div")
                .with_class("title")
                .with_text("Title"),
        );
        root = root.with_child(VirtualNode::new("ul"));

        // Find all following siblings of p.
        let result = query_xpath(&root, "//p/following-sibling::*");
        assert_eq!(result.len(), 2, "Should find 2 following siblings");
        assert_eq!(result[0].tag, "div");
        assert_eq!(result[1].tag, "ul");
    }

    #[test]
    fn query_following_sibling_with_predicate() {
        // Create a DOM with titled blocks.
        let mut root = VirtualNode::new("div").with_class("document");
        root = root.with_child(VirtualNode::new("p").with_text("Para"));
        root = root.with_child(
            VirtualNode::new("div")
                .with_class("literalblock")
                .with_text("Literal"),
        );
        root = root.with_child(VirtualNode::new("p").with_text("Another"));

        // Find following sibling with class="literalblock".
        let result = query_xpath(
            &root,
            "//p[1]/following-sibling::*[@class=\"literalblock\"]",
        );
        assert_eq!(result.len(), 1, "Should find the literalblock element");
        assert_eq!(result[0].text.as_deref(), Some("Literal"));
    }

    #[test]
    fn query_following_sibling_no_matches() {
        // Test when there are no matching following siblings.
        let mut root = VirtualNode::new("div").with_class("document");
        root = root.with_child(VirtualNode::new("p").with_text("Para"));
        root = root.with_child(VirtualNode::new("ul"));

        // The last element has no following siblings.
        let result = query_xpath(&root, "//ul/following-sibling::*");
        assert_eq!(
            result.len(),
            0,
            "Last element should have no following siblings"
        );
    }

    #[test]
    fn query_with_arbitrary_attribute() {
        // Test querying for elements with arbitrary attributes.
        let doc = Parser::default().parse("* Foo\n[start=2]\n. Boo\n* Blech\n");
        let vdom = doc.to_virtual_dom();

        // Find all ol elements with start attribute.
        let result = query_xpath(&vdom, "//ol[@start=\"2\"]");
        assert_eq!(result.len(), 1, "Should find one ol with start=2");

        // Verify the attribute value.
        assert_eq!(result[0].attributes.get("start"), Some(&"2".to_string()));
    }

    #[test]
    fn query_deeply_nested_descendants() {
        // Test for the failing test case: //ul/li[1]//p should find all p elements
        // including those nested in divs.
        let doc = Parser::default().parse(
            "== Lists\n\n* Item one, paragraph one\n+\nItem one, paragraph two\n+\n* Item two\n",
        );
        let vdom = doc.to_virtual_dom();

        // Find all p tags in the document.
        let all_p = query_xpath(&vdom, "//p");
        assert_eq!(
            all_p.len(),
            3,
            "Should find 3 total p tags (2 in first li, 1 in second li)"
        );

        // Find direct children p tags of first li.
        let direct_p = query_xpath(&vdom, "//ul/li[1]/p");
        assert_eq!(direct_p.len(), 1, "Should find 1 direct p child");

        // Find ALL descendant p tags of first li (including nested ones).
        let all_descendants_p = query_xpath(&vdom, "//ul/li[1]//p");
        assert_eq!(
            all_descendants_p.len(),
            2,
            "Should find 2 descendant p tags (direct + nested in div)"
        );
    }

    #[test]
    fn query_with_starts_with_predicate() {
        // Test starts-with() function in predicates.
        let doc = Parser::default().parse("Hello World\n\nGoodbye Moon\n");
        let vdom = doc.to_virtual_dom();

        // Find paragraphs that start with "Hello".
        let result = query_xpath(&vdom, "//p[starts-with(text(),\"Hello\")]");
        assert_eq!(
            result.len(),
            1,
            "Should find one paragraph starting with Hello"
        );
        assert_eq!(result[0].text.as_deref(), Some("Hello World"));

        // Find paragraphs that start with "Good".
        let result = query_xpath(&vdom, "//p[starts-with(text(),\"Good\")]");
        assert_eq!(
            result.len(),
            1,
            "Should find one paragraph starting with Good"
        );
        assert_eq!(result[0].text.as_deref(), Some("Goodbye Moon"));

        // Find paragraphs that start with "Foo" (should find none).
        let result = query_xpath(&vdom, "//p[starts-with(text(),\"Foo\")]");
        assert_eq!(
            result.len(),
            0,
            "Should find no paragraphs starting with Foo"
        );
    }

    #[test]
    fn test_extract_predicates() {
        use super::extract_predicates;

        // Single predicate followed by path.
        let (pred, path) = extract_predicates("[@id=\"beck\"]/div");
        assert_eq!(pred, "[@id=\"beck\"]");
        assert_eq!(path, "/div");

        // Multiple predicates followed by path.
        let (pred, path) = extract_predicates("[@id=\"beck\"][@class=\"title\"]/div/span");
        assert_eq!(pred, "[@id=\"beck\"][@class=\"title\"]");
        assert_eq!(path, "/div/span");

        // Predicates only, no path.
        let (pred, path) = extract_predicates("[@id=\"test\"]");
        assert_eq!(pred, "[@id=\"test\"]");
        assert_eq!(path, "");

        // Path only, no predicates.
        let (pred, path) = extract_predicates("/div/span");
        assert_eq!(pred, "");
        assert_eq!(path, "/div/span");
    }

    #[test]
    fn query_parenthesized_with_predicates_and_child_path() {
        // Test the complex query pattern from the failing test:
        // (//ul/li[1]/p/following-sibling::*)[1][@id="beck"]/div[@class="title"]
        let doc = Parser::default().parse("== Lists\n\n* Item one, paragraph one\n+\n:foo: bar\n[[beck]]\n.Read the following aloud to yourself\n[source, ruby]\n----\n5.times { print \"Odelay!\" }\n----\n\n* Item two\n");
        let vdom = doc.to_virtual_dom();

        // First verify the listingblock with id="beck" exists.
        let listing = query_xpath(&vdom, "//*[@id=\"beck\"]");
        assert_eq!(listing.len(), 1, "Should find the beck listingblock");

        // Verify the div.title child exists.
        let title = query_xpath(&vdom, "//*[@id=\"beck\"]/div[@class=\"title\"]");
        assert_eq!(title.len(), 1, "Should find the title div");

        // Now test the full complex query.
        let result = query_xpath(
            &vdom,
            "(//ul/li[1]/p/following-sibling::*)[1][@id=\"beck\"]/div[@class=\"title\"]",
        );
        assert_eq!(
            result.len(),
            1,
            "Should find the title div via complex query"
        );
        assert!(result[0].text.as_deref().unwrap().starts_with("Read"));
    }
}
