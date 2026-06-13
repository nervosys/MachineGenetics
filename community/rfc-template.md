# RFC 0000 — Title

- **RFC ID**: 0000
- **Author**: Your Name (@github-handle)
- **Status**: Draft
- **Created**: YYYY-MM-DD
- **Updated**: YYYY-MM-DD

## Summary

One paragraph explanation of the proposal.

## Motivation

Why are we doing this? What use cases does it support? What problems does it
solve? What is the expected outcome?

## Design

### Syntax

Show the proposed syntax with examples:

```MAGE
// Example of the proposed feature
```

### Semantics

Describe the behavior precisely:

- What does the feature do?
- How does it interact with existing features?
- What are the edge cases?

### Effect System Impact

Does this proposal affect the effect system? If so, describe:

- New effects introduced
- Changes to effect composition (`/` operator)
- Effect handler implications

### SKB Impact

Does this proposal affect Safety Knowledge Base rules? If so, describe:

- New SKB rules required
- Changes to existing rules
- Ownership / borrowing implications

### Transpiler Impact

How does this translate between Rust and MAGE?

```rust
// Rust equivalent
```

```MAGE
// MAGE version
```

- Changes needed in `rust2mg`
- Changes needed in `mg2rs`

## Examples

### Basic Usage

```MAGE
// Simple example
```

### Advanced Usage

```MAGE
// More complex example showing edge cases
```

## Alternatives Considered

### Alternative A

Description and why it was rejected.

### Alternative B

Description and why it was rejected.

### Do Nothing

What happens if we don't implement this?

## Drawbacks

- What are the downsides?
- Does it add complexity?
- Does it break existing code?

## Compatibility

### Backward Compatibility

Is this a breaking change? If so, what migration path is provided?

### Edition Boundary

Which edition would this land in? Can `mg migrate` handle it automatically?

### Rust Interoperability

Does this affect Rust compatibility? Can `mg2rs` still produce valid Rust?

## Prior Art

How have other languages / systems solved this problem?

- **Rust**: ...
- **Other languages**: ...

## Unresolved Questions

- What parts of the design are still open?
- What related issues are out of scope for this RFC?

## Implementation Plan

1. Parser changes
2. Transpiler rule updates
3. Standard library additions
4. Documentation updates
5. Training data samples
6. Test cases

## Checklist

- [ ] Summary clearly describes the proposal
- [ ] Motivation explains why this is needed
- [ ] Design section covers syntax, semantics, and interactions
- [ ] Examples show basic and advanced usage
- [ ] Alternatives have been considered
- [ ] Compatibility impact is documented
- [ ] Implementation plan is outlined
