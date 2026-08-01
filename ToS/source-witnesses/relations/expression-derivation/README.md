# Expression Derivation Claims

This route owns directed, evidence-bearing `Expression --is_derivative_of-->
Expression` claims. The subject is the derived Expression and the object is
one identified source Expression. `qualifiers` must conform to
`ToS/contracts/expression-derivation.schema.json`.

The relation is non-transitive, asymmetric, irreflexive, and does not imply
equivalence. A missing source endpoint creates no edge: Work-to-Expression
topology is sufficient until the historical derivation chain is evidenced.
The authored claim packet is authority; catalog and graph rows are generated
source-return projections.
