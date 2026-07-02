# Concepts

Shared domain vocabulary for this project — entities, named processes, and status concepts with project-specific meaning. Seeded with core domain vocabulary, then accretes as ce-compound and ce-compound-refresh process learnings; direct edits are fine. Glossary only, not a spec or catch-all.

## Rule Engine

### NodeData
The closed-set enum carrying data between nodes in the execution pipeline. Variants: Raw (intermediate text/JS output), HttpResponse (status/headers/body/charset), Media (BookMedia/Video/Audio), Json (serde_json::Value), Error (propagated failure). Every node's output stream produces NodeData items; every downstream node consumes them. Error is a first-class variant — processors must pass it through, never silently discard it.

### NodeKind
The 6 variants of executable nodes: Http, Js, Extract, Merge, Condition, Loop. Endpoint is not a NodeKind — endpoints are subgraph templates composed of these node kinds. Merge/Condition/Loop are first-class control-flow nodes with stub processors in the first cut.

### NodeProcessor
The trait that transforms an input stream of NodeData into an output stream of NodeData. Signature: `process(&self, ctx, spec, input: BoxStream<NodeData>) -> BoxStream<NodeData>` — sync return, stream-to-stream pipeline. Each NodeKind has one NodeProcessor implementation. Processors must not silently swallow NodeData::Error.

### EndpointKind
The 5 endpoint types that define a rule's user-facing capabilities: Search (keyword → N books), Discover (explore URL → N categories/books), Detail (book URL → book metadata), Toc (book URL → N chapters), Content (chapter URL → chapter text). Each endpoint is a subgraph template (e.g., Http→Extract), not a NodeKind. Execution proceeds in segments controlled by the frontend.

### GraphExecutor
The engine that runs a Graph's stream pipeline. Selects a subgraph by EndpointKind, topologically sorts nodes, wires NodeProcessor outputs to downstream inputs, and wraps output streams with tap (tracing + node-output events). Returns a stream of (NodeId, NodeData) tuples.

### SegmentSpec
The parameter bundle for one execution segment. Carries the EndpointKind plus segment-specific data (query for Search, book_url for Detail/Toc, chapter_url for Content). The frontend issues execute_segment IPC commands per segment — Graph contains no segment boundaries.

### ExtractRule
The IR enum for content extraction rules, populated at import time by the compiler. Variants: CssSelector, XPath, JsonPath, Regex — each with an extract type (text/href/src/html) and optional regex cleanup. Lives in lj-core as a closed set; lj-compiler parses Legado syntax into this IR.

### Graph
The complete DAG representing a rule: nodes (NodeSpec), edges (with optional ConditionBranch), and a subroutine table (HashMap<SubroutineId, Graph> for Loop). Validated against GraphSchema (endpoint subgraph templates + node I/O type constraints) at import time.
