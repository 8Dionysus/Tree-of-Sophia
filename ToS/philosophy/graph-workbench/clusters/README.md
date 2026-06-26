# Graph Clusters

`clusters/` names how a large philosophy graph may collapse into reviewable
groups without losing source refs.

Clusters are source-owned contracts. Runtime surfaces may render, filter, or
cache them, but they do not decide what the cluster means.

Each cluster must be able to expand back to member node ids, related edge ids,
and source refs. When the current atlas lacks a mature field for a future
cluster kind, the generated projection should report an unresolved diagnostic
instead of inventing members.

